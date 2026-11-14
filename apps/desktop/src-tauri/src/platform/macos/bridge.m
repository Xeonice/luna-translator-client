// Native-only platform adapter. No frames are saved or sent to model services.
#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <ImageIO/ImageIO.h>
#import <libproc.h>
#import <objc/runtime.h>
#include <stdatomic.h>
#include <sys/proc_info.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/sysctl.h>
#import <CommonCrypto/CommonDigest.h>

// AppKit AppleEvent/Dock termination bypasses Tao's RequestExit path. Defer
// that native decision until the same Rust shutdown/drain path has committed.
// These values and NSApplication replies are confined to the AppKit thread.
static void (*LunaQuitCallback)(void) = NULL;
static Class LunaQuitDelegateClass = Nil;
static BOOL LunaQuitPending = NO;

static NSApplicationTerminateReply LunaShouldTerminate(id delegate, SEL selector, NSApplication *application) {
    (void)delegate; (void)selector; (void)application;
    if (![NSThread isMainThread] || !LunaQuitCallback) return NSTerminateCancel;
    if (!LunaQuitPending) {
        LunaQuitPending = YES;
        // Return NSTerminateLater before posting the Rust event. No SQLite,
        // worker wait or nested application callback runs on this AppKit stack.
        // AppKit may run a nested modal loop for NSTerminateLater. A main
        // dispatch block cannot re-enter its serial queue in that loop.
        CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopCommonModes, ^{
            if (LunaQuitPending && LunaQuitCallback) LunaQuitCallback();
        });
        CFRunLoopWakeUp(CFRunLoopGetMain());
    }
    return NSTerminateLater;
}

int luna_quit_initialize(void (*callback)(void)) {
    if (![NSThread isMainThread]) return 1;
    id delegate = NSApp.delegate;
    if (!callback || !delegate) return 2;
    Class cls = object_getClass(delegate);
    SEL selector = @selector(applicationShouldTerminate:);
    Method existing = class_getInstanceMethod(cls, selector);
    if (existing) {
        // Preserve every existing/inherited framework method. Only an exact
        // repeat of our own successful installation is accepted.
        return cls == LunaQuitDelegateClass && LunaQuitCallback == callback &&
            method_getImplementation(existing) == (IMP)LunaShouldTerminate ? 0 : 3;
    }
    if (LunaQuitCallback || LunaQuitDelegateClass) return 4;
    NSString *signature = [NSString stringWithFormat:@"%s@:@", @encode(NSApplicationTerminateReply)];
    if (!class_addMethod(cls, selector, (IMP)LunaShouldTerminate, signature.UTF8String)) return 5;
    LunaQuitDelegateClass = cls;
    LunaQuitCallback = callback;
    return 0;
}

int luna_quit_reply(int allow) {
    if (![NSThread isMainThread]) return -1;
    if (!LunaQuitPending) return 0;
    LunaQuitPending = NO;
    [NSApp replyToApplicationShouldTerminate:allow != 0];
    return 1;
}

int luna_quit_reply_async(int allow, void (*completion)(void *, int), void *context) {
    if (![NSThread isMainThread]) return 1;
    if (!completion || !context) return 2;
    // Tauri's main-thread Task holds Tao's callback mutex. A YES reply may
    // synchronously invoke applicationWillTerminate -> Tao AppState::exit,
    // which needs that mutex again. Leave the Task before replying in a
    // separate common-mode RunLoop block, never a reentrant GCD queue block.
    CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopCommonModes, ^{
        int status = luna_quit_reply(allow);
        // For NO this also restores Rust's retry gate before another main
        // event can issue Quit. YES may terminate before returning here; the
        // process then owns disposal of its one pending completion context.
        completion(context, status);
    });
    CFRunLoopWakeUp(CFRunLoopGetMain());
    return 0;
}

@interface LunaMaterialView : NSVisualEffectView
@end
@implementation LunaMaterialView
- (NSInteger)tag { return 0x4c554e41; }
@end

static BOOL AtLeast(NSInteger major, NSInteger minor) {
    return [NSProcessInfo.processInfo isOperatingSystemAtLeastVersion:(NSOperatingSystemVersion){major, minor, 0}];
}

static void OnMain(dispatch_block_t block) {
    if ([NSThread isMainThread]) block(); else dispatch_sync(dispatch_get_main_queue(), block);
}

// Private owner geometry, deliberately separate from game capture and Tao's
// delegate. Observer tokens die with their exact NSWindow instance.
typedef struct {
    uint64_t epoch;
    double x, y;
    uint32_t width, height;
    double scale;
    uint32_t live_resize;
} LunaOwnerGeometry;
static void (*LunaGeometryEdge)(uint64_t, int) = NULL;
static uint64_t LunaGeometryEpoch = 0;
static char LunaGeometryKey;
@interface LunaGeometryObserver : NSObject
@property(nonatomic) uint64_t epoch;
@property(nonatomic) BOOL closed;
@property(nonatomic, strong) NSMutableArray *tokens;
@end
@implementation LunaGeometryObserver
- (void)dealloc {
    for (id token in _tokens) [NSNotificationCenter.defaultCenter removeObserver:token];
}
@end
int luna_geometry_initialize(void (*callback)(uint64_t, int)) {
    if (!callback || (LunaGeometryEdge && LunaGeometryEdge != callback)) return 1;
    LunaGeometryEdge = callback;
    return 0;
}
static LunaGeometryObserver *GeometryObserver(NSWindow *window) {
    LunaGeometryObserver *observer = objc_getAssociatedObject(window, &LunaGeometryKey);
    if (observer) return observer;
    observer = [LunaGeometryObserver new];
    observer.epoch = ++LunaGeometryEpoch;
    observer.tokens = [NSMutableArray array];
    __weak LunaGeometryObserver *weakObserver = observer;
    NSArray *names = @[NSWindowWillStartLiveResizeNotification, NSWindowDidEndLiveResizeNotification, NSWindowWillCloseNotification];
    for (NSUInteger index = 0; index < names.count; index++) {
        int edge = (int)index + 1;
        id token = [NSNotificationCenter.defaultCenter addObserverForName:names[index] object:window queue:nil usingBlock:^(NSNotification *notification) {
            (void)notification;
            LunaGeometryObserver *current = weakObserver;
            if (!current || current.closed) return;
            if (edge == 3) current.closed = YES;
            // Rust only records an edge and queues work here. No nested native
            // destroy/setFrame or blocking application work on this stack.
            if (LunaGeometryEdge) LunaGeometryEdge(current.epoch, edge);
        }];
        [observer.tokens addObject:token];
    }
    objc_setAssociatedObject(window, &LunaGeometryKey, observer, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    return observer;
}
static int ReadOwnerGeometry(NSWindow *window, LunaOwnerGeometry *result) {
    if (!window || !result || ![NSThread isMainThread]) return 1;
    LunaGeometryObserver *observer = GeometryObserver(window);
    if (observer.closed || !window.contentView) return 2;
    double scale = window.backingScaleFactor;
    NSRect frame = window.frame;
    NSRect content = [window.contentView convertRectToBacking:window.contentView.bounds];
    if (!isfinite(scale) || scale <= 0 || !isfinite(content.size.width) || !isfinite(content.size.height)) return 3;
    *result = (LunaOwnerGeometry){observer.epoch, round(frame.origin.x * scale), round(NSMaxY(frame) * scale), (uint32_t)llround(content.size.width), (uint32_t)llround(content.size.height), scale, window.inLiveResize ? 1 : 0};
    return 0;
}
int luna_owner_geometry(void *rawWindow, LunaOwnerGeometry *result) {
    __block int status = 1;
    OnMain(^{ status = ReadOwnerGeometry((__bridge NSWindow *)rawWindow, result); });
    return status;
}
int luna_geometry_defer(void (*callback)(void *), void *context) {
    if (!callback || !context) return 1;
    // Unlike a Tauri main-thread Task, this leaves the Tao callback mutex and
    // the current AppKit notification before executing application effects.
    CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopCommonModes, ^{ callback(context); });
    CFRunLoopWakeUp(CFRunLoopGetMain());
    return 0;
}
int luna_hud_content_fit(void *rawWindow, const LunaOwnerGeometry *before, uint32_t height, LunaOwnerGeometry *after) {
    if (![NSThread isMainThread] || !before || !after) return 1;
    NSWindow *window = (__bridge NSWindow *)rawWindow;
    LunaOwnerGeometry actual = {0};
    if (ReadOwnerGeometry(window, &actual)) return 2;
    if (actual.epoch != before->epoch || actual.scale != before->scale || actual.x != before->x || actual.y != before->y || actual.width != before->width || actual.height != before->height || actual.live_resize) return 3;
    @try {
        NSRect oldFrame = window.frame;
        NSRect content = [window contentRectForFrameRect:oldFrame];
        content.size.height = height / actual.scale;
        NSRect next = [window frameRectForContentRect:content];
        next.origin.x = oldFrame.origin.x;
        next.origin.y = NSMaxY(oldFrame) - next.size.height;
        [window setFrame:next display:YES animate:NO];
    } @catch (NSException *exception) {
        (void)exception;
        ReadOwnerGeometry(window, after);
        return 4;
    }
    // Completion means native apply plus actual readback, not a queued setter.
    return ReadOwnerGeometry(window, after) ? 5 : 0;
}
static char *JSON(id value) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    if (!data) return NULL;
    char *result = malloc(data.length + 1);
    if (result) { memcpy(result, data.bytes, data.length); result[data.length] = 0; }
    return result;
}
void luna_native_free(void *value) { free(value); }
static NSString *ProcessStart(pid_t pid) {
    struct proc_bsdinfo info = {0};
    if (proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != sizeof(info)) return nil;
    return [NSString stringWithFormat:@"%llu:%llu", info.pbi_start_tvsec, info.pbi_start_tvusec];
}
int luna_preview_supported(void) { if (AtLeast(14, 0)) return 1; return 0; }
int luna_preview_permission(int request) {
    if (!request) return CGPreflightScreenCaptureAccess();
    __block int granted = 0;
    OnMain(^{ granted = CGPreflightScreenCaptureAccess(); if (!granted && request) granted = CGRequestScreenCaptureAccess(); });
    return granted;
}
int luna_reduce_transparency(void) {
    __block int result = 1;
    OnMain(^{ result = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency; });
    return result;
}
// Verify the actual view configuration on AppKit's thread, rather than treating
// Tauri's queued set_effects receipt as a material effect receipt.
int luna_hud_material(void *rawWindow, int reduced) {
    if (!rawWindow) return -1;
    NSWindow *window = (__bridge NSWindow *)rawWindow;
    __block int result = -1;
    OnMain(^{
        NSView *content = window.contentView;
        if (!content) return;
        NSView *existing = [content viewWithTag:0x4c554e41];
        if (!reduced && [existing isKindOfClass:NSVisualEffectView.class]) {
            NSVisualEffectView *effect = (NSVisualEffectView *)existing;
            result = effect.superview == content && effect.state == NSVisualEffectStateActive && effect.material == NSVisualEffectMaterialHUDWindow ? 1 : -1;
            return;
        }
        for (NSView *view in [content.subviews copy]) {
            if ([view isKindOfClass:NSVisualEffectView.class]) [view removeFromSuperview];
        }
        if (reduced) { result = 0; return; }
        NSVisualEffectView *material = [[LunaMaterialView alloc] initWithFrame:content.bounds];
        material.material = NSVisualEffectMaterialHUDWindow;
        material.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        material.state = NSVisualEffectStateActive;
        material.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        material.wantsLayer = YES; material.layer.cornerRadius = 16;
        [content addSubview:material positioned:NSWindowBelow relativeTo:nil];
        result = material.superview == content && material.state == NSVisualEffectStateActive ? 1 : -1;
    });
    return result;
}
char *luna_screens_json(void) {
    __block char *result = NULL;
    OnMain(^{
        NSArray<NSScreen *> *screens = NSScreen.screens;
        CGFloat primaryTop = screens.count ? NSMaxY(screens[0].frame) : 0;
        NSMutableArray *items = [NSMutableArray array];
        for (NSScreen *screen in screens) {
            NSRect rect = screen.visibleFrame;
            [items addObject:@{ @"id": [screen.deviceDescription[@"NSScreenNumber"] stringValue] ?: @"unknown",
                @"workArea": @{ @"x": @(rect.origin.x), @"y": @(primaryTop - NSMaxY(rect)), @"width": @(rect.size.width), @"height": @(rect.size.height) },
                @"scaleFactor": @(screen.backingScaleFactor) }];
        }
        result = JSON(items);
    });
    return result;
}
char *luna_choose_export_path(const char *defaultName) {
    __block char *result = NULL;
    OnMain(^{
        NSSavePanel *panel = [NSSavePanel savePanel];
        panel.nameFieldStringValue = [NSString stringWithUTF8String:defaultName];
        panel.canCreateDirectories = YES;
        panel.extensionHidden = NO;
        if ([panel runModal] == NSModalResponseOK && panel.URL.isFileURL) result = JSON(@{@"path":panel.URL.path});
        else result = JSON(@{@"path":NSNull.null});
    });
    return result;
}
int luna_write_clipboard(const unsigned char *bytes, size_t length) {
    NSString *text = [[NSString alloc] initWithBytes:bytes length:length encoding:NSUTF8StringEncoding];
    if (!text) return 0;
    __block BOOL result = NO;
    OnMain(^{
        NSPasteboard *board = NSPasteboard.generalPasteboard;
        [board clearContents];
        result = [board setString:text forType:NSPasteboardTypeString];
    });
    return result;
}

// A server-generated title challenge attests a Win32 HWND to one native window.
// A normal title is never accepted as an identity proof.
char *luna_resolve_window_nonce(const char *nonce) {
    @autoreleasepool {
        if (!nonce || !CGPreflightScreenCaptureAccess()) return JSON(@{@"error": @"preview_permission_denied"});
        NSString *expected = [NSString stringWithUTF8String:nonce];
        NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionAll, kCGNullWindowID));
        NSMutableArray *matches = [NSMutableArray array];
        for (NSDictionary *item in windows) {
            if ([item[(id)kCGWindowName] isEqualToString:expected] && [item[(id)kCGWindowLayer] intValue] == 0) [matches addObject:item];
        }
        if (matches.count != 1) return JSON(@{@"error": matches.count ? @"ambiguous_capture_window" : @"no_capture_window"});
        NSDictionary *window = matches[0];
        int pid = [window[(id)kCGWindowOwnerPID] intValue];
        NSString *start = ProcessStart(pid);
        if (!start) return JSON(@{@"error": @"host_identity_unavailable"});
        return JSON(@{ @"hostPid": @(pid), @"hostProcessStart": start,
            @"windowId": window[(id)kCGWindowNumber], @"onScreen": window[(id)kCGWindowIsOnscreen] ?: @NO });
    }
}

@interface LunaCaptureResult : NSObject
@property NSData *data;
@property NSString *error;
@property uint64_t width;
@property uint64_t height;
@property uint64_t capturedAt;
@property uint32_t configuredWidth;
@property uint32_t configuredHeight;
@property NSString *reason;
@property NSString *scDomain;
@property long long scCode;
@property uint64_t encodedBytes;
@property NSString *api;
@property NSDictionary *geometry;
@property NSString *failureKind;
@end
@implementation LunaCaptureResult
@end
static int CaptureFailureCode(NSError *failure) {
    if(!failure||![failure.domain isEqualToString:SCStreamErrorDomain])return 6;
    switch(failure.code) {
        case SCStreamErrorInternalError:return 9;
        case SCStreamErrorUserDeclined:
        case SCStreamErrorMissingEntitlements:
        case SCStreamErrorUserStopped:
        case SCStreamErrorSystemStoppedStream:return 10;
        default:return 6;
    }
}
// Bounded local-cover codec. No paths, URLs, screen capture or retained caller buffers.
static CGImageSourceRef CoverSource(const unsigned char *bytes, size_t length, uint32_t *width, uint32_t *height) {
    if (!bytes || !length || length > 8388608) return NULL;
    NSData *data = [NSData dataWithBytes:bytes length:length];
    CGImageSourceRef source = CGImageSourceCreateWithData((__bridge CFDataRef)data, (__bridge CFDictionaryRef)@{(id)kCGImageSourceShouldCache: @NO});
    if (!source) return NULL;
    CFStringRef type = CGImageSourceGetType(source);
    if (!type || (!CFEqual(type, CFSTR("public.jpeg")) && !CFEqual(type, CFSTR("public.png"))) || CGImageSourceGetCount(source) != 1) { CFRelease(source); return NULL; }
    NSDictionary *properties = CFBridgingRelease(CGImageSourceCopyPropertiesAtIndex(source, 0, NULL));
    uint64_t w = [properties[(id)kCGImagePropertyPixelWidth] unsignedLongLongValue];
    uint64_t h = [properties[(id)kCGImagePropertyPixelHeight] unsignedLongLongValue];
    if (w < 160 || h < 90 || w > 8192 || h > 8192 || w * h > 32000000) { CFRelease(source); return NULL; }
    *width = (uint32_t)w; *height = (uint32_t)h; return source;
}
int luna_cover_dimensions(const unsigned char *bytes, size_t length, uint32_t *width, uint32_t *height) {
    @autoreleasepool {
        CGImageSourceRef source = CoverSource(bytes, length, width, height);
        if (!source) return 1; CFRelease(source); return 0;
    }
}
int luna_preview_identity_current(uint32_t windowId, int hostPid, const char *hostStart) {
    @autoreleasepool {
        NSString *expected = hostStart ? [NSString stringWithUTF8String:hostStart] : nil;
        if (!expected || ![ProcessStart(hostPid) isEqualToString:expected]) return 0;
        NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, windowId));
        for (NSDictionary *window in windows) {
            if ([window[(id)kCGWindowNumber] unsignedIntValue] == windowId && [window[(id)kCGWindowOwnerPID] intValue] == hostPid) return 1;
        }
        return 0;
    }
}
// Explicit user pairing. This registry owns the original picker filter; a
// random transaction capability never crosses the public application port.
@interface LunaPickedWindow : NSObject
@property SCContentFilter *filter;
@property NSDictionary *identity;
@property NSDictionary *evidence;
@property NSLock *lock;
@end
@implementation LunaPickedWindow
@end
static NSLock *PickedLock;
static NSMutableDictionary<NSString *, LunaPickedWindow *> *PickedWindows;
static NSMutableDictionary<NSString *, NSNumber *> *PickerTransactions;
static NSMutableDictionary<NSString *, NSMutableDictionary *> *PickerDiagnostics;
static NSDictionary *LastCaptureDiagnostic;
static NSDictionary *LastCaptureDiagnosticIdentity;
static atomic_ullong DiagnosticSequence = 0;
static atomic_bool PickerReserved = false;
static void PickedInitialize(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{ PickedLock = [NSLock new]; PickedWindows = [NSMutableDictionary new]; PickerTransactions = [NSMutableDictionary new]; PickerDiagnostics = [NSMutableDictionary new]; });
}
// Shared receipt operations. A queued cancel owns its bit independently of
// picker completion, so a racing successful callback cannot acknowledge the
// later queued filter cleanup early.
static BOOL PickerRegisterTransaction(NSString *token) {
    PickedInitialize(); [PickedLock lock];
    BOOL accepted = token && PickerTransactions.count < 8 && !PickerTransactions[token];
    if (accepted) {
        PickerTransactions[token] = @1;
        unsigned long long operation = atomic_fetch_add(&DiagnosticSequence, 1) + 1;
        PickerDiagnostics[token] = [@{@"stage":@"picker_accepted", @"operation":@(operation), @"picker_operation":@(operation), @"picker_stage":@"picker_accepted", @"revision":@1, @"received":@0, @"processed":@0} mutableCopy];
    }
    [PickedLock unlock]; return accepted;
}
static void PickerQueueCancellation(NSString *token) {
    PickedInitialize(); [PickedLock lock];
    if (PickerTransactions[token]) PickerTransactions[token] = @([PickerTransactions[token] unsignedIntValue] | 2U);
    [PickedLock unlock];
}
static void PickerCompleteReceiptPart(NSString *token, unsigned part) {
    [PickedLock lock];
    if (PickerTransactions[token]) PickerTransactions[token] = @([PickerTransactions[token] unsignedIntValue] & ~part);
    [PickedLock unlock];
}
static LunaPickedWindow *PickedLookup(NSString *token) {
    PickedInitialize(); [PickedLock lock]; LunaPickedWindow *record = PickedWindows[token]; [PickedLock unlock]; return record;
}
// -1 is unknown, never an acknowledgement. Accepted transactions are
// registered synchronously before main-queue dispatch; 0 consumes a completed
// receipt only after observer/config/reservation cleanup and callback finish.
int luna_preview_picker_transaction_active(const char *token) {
    if (!token) return -1; PickedInitialize();
    NSString *key = [NSString stringWithUTF8String:token]; if (!key) return -1;
    [PickedLock lock]; NSNumber *active = PickerTransactions[key];
    int result = active ? (active.unsignedIntValue ? 1 : 0) : -1;
    if (result == 0) { [PickerTransactions removeObjectForKey:key]; [PickerDiagnostics removeObjectForKey:key]; }
    [PickedLock unlock]; return result;
}
// Diagnostics share the bounded accepted-transaction registry. They are copied
// before active()==0 consumes its receipt; no raw capability leaves this API.
static void PickerDiagnosticEvent(NSString *token, NSString *stage, BOOL received, BOOL processed, BOOL stream, BOOL current) {
    PickedInitialize(); [PickedLock lock]; NSMutableDictionary *value = PickerDiagnostics[token];
    if (value) {
        value[@"stage"] = stage; value[@"picker_stage"] = stage;
        value[@"revision"] = @([value[@"revision"] unsignedLongLongValue] + 1);
        value[@"received"] = @(MIN(255U, [value[@"received"] unsignedIntValue] + (received ? 1U : 0U)));
        value[@"processed"] = @(MIN(255U, [value[@"processed"] unsignedIntValue] + (processed ? 1U : 0U)));
        value[@"stream"] = @(stream); value[@"current"] = @(current);
        if ([stage isEqualToString:@"picker_native_timeout"]) value[@"timeout_source"] = @"native";
    }
    [PickedLock unlock];
}
char *luna_preview_picker_diagnostic(const char *token) {
    if (!token || strlen(token) > 112) return JSON(@{});
    PickedInitialize(); NSString *key = [NSString stringWithUTF8String:token]; if (!key) return JSON(@{});
    [PickedLock lock]; NSDictionary *value = [PickerDiagnostics[key] copy]; [PickedLock unlock];
    return JSON(value ?: @{});
}
char *luna_preview_capture_diagnostic(uint32_t window, int pid, const char *start) {
    if (!start) return JSON(@{}); PickedInitialize(); NSString *expected = [NSString stringWithUTF8String:start];
    [PickedLock lock]; NSDictionary *identity = LastCaptureDiagnosticIdentity;
    NSDictionary *value = [identity[@"window"] unsignedIntValue] == window && [identity[@"pid"] intValue] == pid && [identity[@"start"] isEqualToString:expected] ? LastCaptureDiagnostic : nil;
    [PickedLock unlock]; return JSON(value ?: @{});
}
void luna_preview_selection_release(const char *token) {
    if (!token) return; PickedInitialize(); NSString *key = [NSString stringWithUTF8String:token];
    [PickedLock lock]; [PickedWindows removeObjectForKey:key]; [PickedLock unlock];
}
typedef void (*LunaPickerCompletion)(void *, char *);
static atomic_uint PickerReceivedDiagnostics = 0;
static void PickerCallbackReceived(NSString *token, const char *event, BOOL hasStream) {
    NSString *stage = [NSString stringWithFormat:@"picker_%s_received", event];
    PickerDiagnosticEvent(token, stage, YES, NO, hasStream, NO);
    if (atomic_fetch_add(&PickerReceivedDiagnostics, 1) >= 24) return;
    NSLog(@"LunaPreviewPicker callback_received=%s pid=%d stream=%d enteredMain=%d", event, getpid(), hasStream, NSThread.isMainThread);
}
@interface LunaPickerOperation : NSObject <SCContentSharingPickerObserver>
@property NSString *token;
@property LunaPickerCompletion completion;
@property void *context;
@property BOOL finished;
@property NSUInteger diagnosticCount;
@property SCContentSharingPickerConfiguration *previousConfiguration;
@property BOOL previousActive;
- (void)finish:(NSDictionary *)result;
- (void)record:(NSString *)event stream:(BOOL)stream enteredMain:(BOOL)enteredMain;
@end
static LunaPickerOperation *CurrentPicker;
// Pure callback ownership policy shared by the actual observer and permanent
// Native harness: late operations never finish a newer transaction.
static int PickerCallbackDisposition(BOOL current, BOOL finished, BOOL hasStream) {
    if (!current || finished) return 0;
    return hasStream ? 2 : 1;
}
int luna_preview_picker_callback_policy(int current, int finished, int stream) {
    return PickerCallbackDisposition(current != 0, finished != 0, stream != 0);
}
@implementation LunaPickerOperation
- (void)record:(NSString *)event stream:(BOOL)stream enteredMain:(BOOL)enteredMain {
    NSString *stage = [event isEqualToString:@"native_timeout"] ? @"picker_native_timeout" : [@"picker_" stringByAppendingString:event];
    PickerDiagnosticEvent(self.token, stage, NO, ![event isEqualToString:@"native_timeout"], stream, CurrentPicker == self && !self.finished);
    if (self.diagnosticCount++ >= 8) return;
    // Only fixed enum values and booleans/PID. No title, token, user content,
    // NSError text, paths or process environment enters system diagnostics.
    NSLog(@"LunaPreviewPicker event=%@ pid=%d current=%d finished=%d stream=%d enteredMain=%d timeoutSource=%@",
        event, getpid(), CurrentPicker == self, self.finished, stream, enteredMain,
        [event isEqualToString:@"native_timeout"] ? @"native" : @"none");
}
- (void)finish:(NSDictionary *)result {
    if (self.finished || CurrentPicker != self) return; self.finished = YES;
    SCContentSharingPicker *picker = SCContentSharingPicker.sharedPicker;
    [picker removeObserver:self]; picker.defaultConfiguration = self.previousConfiguration;
    picker.active = self.previousActive;
    if (CurrentPicker == self) CurrentPicker = nil;
    atomic_store(&PickerReserved, false);
    self.completion(self.context, JSON(result)); self.context = NULL;
    PickerCompleteReceiptPart(self.token, 1U);
}
- (void)contentSharingPicker:(SCContentSharingPicker *)picker didCancelForStream:(SCStream *)stream {
    (void)picker; BOOL enteredMain = NSThread.isMainThread; BOOL hasStream = stream != nil;
    PickerCallbackReceived(self.token, "cancel", hasStream);
    dispatch_async(dispatch_get_main_queue(), ^{
        [self record:@"cancel_processed" stream:hasStream enteredMain:enteredMain];
        int disposition = PickerCallbackDisposition(CurrentPicker == self, self.finished, hasStream);
        if (!disposition) return;
        [self finish:@{@"error": disposition == 2 ? @"preview_selection_failed" : @"preview_selection_cancelled"}];
    });
}
- (void)contentSharingPickerStartDidFailWithError:(NSError *)error {
    (void)error; BOOL enteredMain = NSThread.isMainThread;
    PickerCallbackReceived(self.token, "start_failed", NO);
    dispatch_async(dispatch_get_main_queue(), ^{
        [self record:@"start_failed_processed" stream:NO enteredMain:enteredMain];
        if (!PickerCallbackDisposition(CurrentPicker == self, self.finished, NO)) return;
        [self finish:@{@"error":@"preview_selection_failed"}];
    });
}
- (void)contentSharingPicker:(SCContentSharingPicker *)picker didUpdateWithFilter:(SCContentFilter *)filter forStream:(SCStream *)stream {
    (void)picker; BOOL enteredMain = NSThread.isMainThread; BOOL hasStream = stream != nil;
    PickerCallbackReceived(self.token, "update", hasStream);
    dispatch_async(dispatch_get_main_queue(), ^{
        [self record:@"update_processed" stream:hasStream enteredMain:enteredMain];
        int disposition = PickerCallbackDisposition(CurrentPicker == self, self.finished, hasStream);
        if (!disposition) return;
        if (disposition == 2) { [self finish:@{@"error":@"preview_selection_failed"}]; return; }
        if (AtLeast(15, 2)) {
            if (filter.style != SCShareableContentStyleWindow || filter.includedWindows.count != 1) {
                [self finish:@{@"error":@"preview_selection_mismatch"}]; return;
            }
            SCWindow *window = filter.includedWindows.firstObject; pid_t pid = window.owningApplication.processID;
            NSString *start = ProcessStart(pid);
            if (!window.windowID || pid <= 0 || pid == getpid() || !start) {
                [self finish:@{@"error":@"preview_selection_mismatch"}]; return;
            }
            LunaPickedWindow *record = [LunaPickedWindow new]; record.filter = filter; record.lock = [NSLock new];
            record.identity = @{@"hostPid":@(pid), @"hostProcessStart":start, @"windowId":@(window.windowID), @"onScreen":@(window.onScreen)};
            PickedInitialize(); [PickedLock lock];
            if (PickedWindows.count >= 4) { [PickedLock unlock]; [self finish:@{@"error":@"preview_lease_limit"}]; return; }
            PickedWindows[self.token] = record; [PickedLock unlock];
            [self finish:record.identity];
        } else [self finish:@{@"error":@"preview_selection_unsupported"}];
    });
}
@end
int luna_preview_select_begin(const char *transaction, LunaPickerCompletion completion, void *context) {
    if (!transaction || !completion || strlen(transaction) > 112) return 1;
    if (!AtLeast(15, 2)) return 2;
    if (!CGPreflightScreenCaptureAccess()) return 3;
    bool expected = false; if (!atomic_compare_exchange_strong(&PickerReserved, &expected, true)) return 4;
    NSString *token = [NSString stringWithUTF8String:transaction];
    if (!PickerRegisterTransaction(token)) { atomic_store(&PickerReserved, false); return 4; }
    dispatch_async(dispatch_get_main_queue(), ^{
        LunaPickerOperation *operation = [LunaPickerOperation new]; operation.token = token; operation.completion = completion; operation.context = context;
        SCContentSharingPicker *picker = SCContentSharingPicker.sharedPicker;
        operation.previousConfiguration = picker.defaultConfiguration; operation.previousActive = picker.active;
        CurrentPicker = operation;
        SCContentSharingPickerConfiguration *config = [SCContentSharingPickerConfiguration new];
        config.allowedPickerModes = SCContentSharingPickerModeSingleWindow; config.allowsChangingSelectedContent = NO;
        NSString *bundle = NSBundle.mainBundle.bundleIdentifier; if (bundle) config.excludedBundleIDs = @[bundle];
        picker.defaultConfiguration = config; [picker addObserver:operation]; picker.active = YES;
        [picker presentPickerUsingContentStyle:SCShareableContentStyleWindow];
        PickerDiagnosticEvent(token, @"picker_presented", NO, NO, NO, YES);
        __weak LunaPickerOperation *weakOperation = operation;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 90 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
            LunaPickerOperation *live = weakOperation;
            if (live && PickerCallbackDisposition(CurrentPicker == live, live.finished, NO)) {
                [live record:@"native_timeout" stream:NO enteredMain:NSThread.isMainThread];
                [live finish:@{@"error":@"preview_selection_timeout"}];
            }
        });
    });
    return 0; // accepted: exactly one completion now owns the context
}
void luna_preview_select_cancel(const char *transaction) {
    if (!transaction) return; NSString *token = [NSString stringWithUTF8String:transaction];
    PickerQueueCancellation(token);
    dispatch_async(dispatch_get_main_queue(), ^{
        LunaPickerOperation *operation = CurrentPicker;
        if ([operation.token isEqualToString:token]) [operation finish:@{@"error":@"preview_selection_cancelled"}];
        luna_preview_selection_release(token.UTF8String);
        PickerCompleteReceiptPart(token, 2U);
    });
}

// Parse only stable, bounded filesystem facts; do not retain or emit process
// environments. Read-only PE header mappings are deliberately not execute proof.
static NSString *FileDigest(NSData *data) {
    unsigned char digest[CC_SHA256_DIGEST_LENGTH]; CC_SHA256(data.bytes, (CC_LONG)data.length, digest);
    NSMutableString *text = [NSMutableString string]; for (unsigned i=0; i<sizeof(digest); i++) [text appendFormat:@"%02x",digest[i]]; return text;
}
static NSDictionary *FileStamp(const struct stat *s) {
    return @{@"dev":@((uint64_t)s->st_dev), @"ino":@((uint64_t)s->st_ino), @"size":@((uint64_t)s->st_size), @"mtimeSec":@(s->st_mtimespec.tv_sec), @"mtimeNano":@(s->st_mtimespec.tv_nsec)};
}
// Directory children may change normally (for example Wine's atomic registry
// save). Bottle authority depends on the canonical directory, not its mtime.
static NSDictionary *BottleIdentity(NSString *path) {
    if(!path.length)return nil;
    char *resolved=realpath(path.fileSystemRepresentation,NULL);if(!resolved)return nil;
    NSString *canonical=[NSFileManager.defaultManager stringWithFileSystemRepresentation:resolved length:strlen(resolved)];free(resolved);
    struct stat before,after;
    if(stat(canonical.fileSystemRepresentation,&before)||!S_ISDIR(before.st_mode))return nil;
    char *rechecked=realpath(path.fileSystemRepresentation,NULL);if(!rechecked)return nil;
    NSString *again=[NSFileManager.defaultManager stringWithFileSystemRepresentation:rechecked length:strlen(rechecked)];free(rechecked);
    if(![canonical isEqualToString:again]||stat(again.fileSystemRepresentation,&after)||!S_ISDIR(after.st_mode)
        ||before.st_dev!=after.st_dev||before.st_ino!=after.st_ino)return nil;
    return @{@"canonicalRoot":canonical,@"dev":@((uint64_t)before.st_dev),@"ino":@((uint64_t)before.st_ino),@"type":@"directory"};
}
char *luna_preview_bottle_identity_for_test(const char *path) {
    @autoreleasepool {
        NSDictionary *identity=path?BottleIdentity([NSString stringWithUTF8String:path]):nil;
        return JSON(identity?:@{@"error":@"bottle_identity_unavailable"});
    }
}
static NSData *StableRead(NSString *path, NSUInteger limit, NSDictionary **stamp) {
    if(!path.length||!stamp)return nil;
    NSString *canonical = path.stringByResolvingSymlinksInPath;
    int fd = open(canonical.fileSystemRepresentation, O_RDONLY|O_CLOEXEC|O_NOFOLLOW); if (fd < 0) return nil;
    struct stat before, after, current;
    if (fstat(fd,&before) || !S_ISREG(before.st_mode) || before.st_size < 0 || (uint64_t)before.st_size > limit) { close(fd); return nil; }
    NSMutableData *data = [NSMutableData dataWithLength:(NSUInteger)before.st_size]; size_t offset=0;
    while (offset < data.length) { ssize_t n=pread(fd,(char *)data.mutableBytes+offset,data.length-offset,(off_t)offset); if (n<=0) {close(fd);return nil;} offset+=(size_t)n; }
    BOOL stable = !fstat(fd,&after) && !stat(path.fileSystemRepresentation,&current)
        && [FileStamp(&before) isEqual:FileStamp(&after)] && [FileStamp(&before) isEqual:FileStamp(&current)]
        && [canonical isEqualToString:path.stringByResolvingSymlinksInPath]; close(fd);
    if (!stable) return nil;
    NSMutableDictionary *facts = [FileStamp(&before) mutableCopy]; facts[@"hash"] = FileDigest(data); *stamp = facts; return data;
}
@interface LunaPlistKeys : NSObject <NSXMLParserDelegate>
@property NSMutableDictionary<NSString *,NSNumber *> *counts;
@property NSMutableString *key;
@end
@implementation LunaPlistKeys
- (void)parser:(NSXMLParser *)parser didStartElement:(NSString *)name namespaceURI:(NSString *)uri qualifiedName:(NSString *)qualified attributes:(NSDictionary *)attributes {
    (void)uri;(void)qualified;(void)attributes; if ([name isEqualToString:@"key"]) {if(self.key){[parser abortParsing];return;}self.key=[NSMutableString new];}
}
- (void)parser:(NSXMLParser *)parser foundCharacters:(NSString *)string { if(self.key.length+string.length>4096){[parser abortParsing];return;}[self.key appendString:string]; }
- (void)parser:(NSXMLParser *)parser didEndElement:(NSString *)name namespaceURI:(NSString *)uri qualifiedName:(NSString *)qualified {
    (void)uri;(void)qualified; if ([name isEqualToString:@"key"]) {if(!self.key||self.counts.count>4096){[parser abortParsing];return;} self.counts[self.key]=@([self.counts[self.key] unsignedIntegerValue]+1); self.key=nil; }
}
@end
static NSDictionary *UniqueHelperPlist(NSData *data) {
    if(!data.length)return nil;
    LunaPlistKeys *keys=[LunaPlistKeys new];keys.counts=[NSMutableDictionary new]; NSXMLParser *parser=[[NSXMLParser alloc] initWithData:data];parser.delegate=keys;parser.shouldResolveExternalEntities=NO;
    if (![parser parse]) return nil;
    for (NSString *name in @[@"CFBundleExecutable",@"CFBundleIdentifier",@"CXHelperAppBottleName",@"CXHelperAppBottleTag"]) if ([keys.counts[name] integerValue] != 1) return nil;
    id value=[NSPropertyListSerialization propertyListWithData:data options:NSPropertyListImmutable format:NULL error:NULL];return [value isKindOfClass:NSDictionary.class]?value:nil;
}
static NSString *ConfBottleID(NSData *data) {
    if(!data.length)return nil;
    NSString *text=[[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding]; if (!text) return nil;
    NSString *section=nil,*value=nil; unsigned count=0;
    for (NSString *raw in [text componentsSeparatedByCharactersInSet:NSCharacterSet.newlineCharacterSet]) {
        NSString *line=[raw stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceCharacterSet];
        if ([line hasPrefix:@"["] && [line hasSuffix:@"]"]) { section=line;continue; }
        if (![section isEqualToString:@"[Bottle]"] || ![line hasPrefix:@"\"BottleID\""]) continue;
        NSArray *parts=[line componentsSeparatedByString:@"="]; if (parts.count!=2) return nil;
        if(![[parts[0] stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceCharacterSet] isEqualToString:@"\"BottleID\""])return nil;
        NSString *right=[parts[1] stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceCharacterSet];
        if (right.length!=38 || ![right hasPrefix:@"\""] || ![right hasSuffix:@"\""]) return nil;
        NSUUID *uuid=[[NSUUID alloc] initWithUUIDString:[right substringWithRange:NSMakeRange(1,36)]]; if (!uuid) return nil;
        value=uuid.UUIDString;count++;
    }
    return count==1?value:nil;
}
static NSString *OwnerPrefix(pid_t pid, BOOL *valid) {
    *valid=NO; int mib[]={CTL_KERN,KERN_PROCARGS2,pid};size_t size=0;
    if (sysctl(mib,3,NULL,&size,NULL,0) || size<sizeof(int) || size>1048576) return nil;
    NSMutableData *data=[NSMutableData dataWithLength:size];if (sysctl(mib,3,data.mutableBytes,&size,NULL,0))return nil;
    const char *bytes=data.bytes;int argc=0;memcpy(&argc,bytes,sizeof(argc));if(argc<1||argc>4096)return nil;
    size_t cursor=sizeof(argc);while(cursor<size&&bytes[cursor])cursor++;if(cursor==size)return nil;
    while(cursor<size&&!bytes[cursor])cursor++;
    for(int arg=0;arg<argc;arg++){while(cursor<size&&bytes[cursor])cursor++;if(cursor==size)return nil;cursor++;}
    NSString *prefix=nil;unsigned found=0;
    while(cursor<size){size_t start=cursor;while(cursor<size&&bytes[cursor])cursor++;if(cursor==size)return nil;
        if(cursor>start && cursor-start>=11 && !memcmp(bytes+start,"WINEPREFIX=",11)){found++;prefix=[[NSString alloc] initWithBytes:bytes+start+11 length:cursor-start-11 encoding:NSUTF8StringEncoding];}
        cursor++;
    }
    *valid=found<=1 && (!found || prefix.length>0); return prefix;
}
// This is an existential mistake filter, not a complete VM inventory or proof
// that the owner executes this PE. A positive exact-vnode witness is enough;
// exhausting Darwin's address space can legitimately return 0/EINVAL.
static BOOL OwnerMapsImage(pid_t pid, const struct stat *image) {
    uint64_t address=0;CFAbsoluteTime deadline=CFAbsoluteTimeGetCurrent()+0.25;
    for(unsigned i=0;i<8192;i++){
        if(CFAbsoluteTimeGetCurrent()>deadline)return NO;
        struct proc_regionwithpathinfo region={0};int n=proc_pidinfo(pid,PROC_PIDREGIONPATHINFO,address,&region,sizeof(region));
        if(n!=sizeof(region))return NO;
        const struct vinfo_stat *v=&region.prp_vip.vip_vi.vi_stat;
        if(v->vst_dev==(uint32_t)image->st_dev&&v->vst_ino==image->st_ino)return YES;
        uint64_t next=region.prp_prinfo.pri_address+region.prp_prinfo.pri_size;if(next<=address)return NO;address=next;
    }
    return NO;
}
// Read-only test seam for the production predicate. Rust exposes this symbol
// only under cfg(test); neither window selection nor public IPC accepts an FD.
int luna_preview_owner_maps_file(int pid, int imageFD) {
    @autoreleasepool {
        struct stat before,after;
        if(pid<=0||fstat(imageFD,&before)||!S_ISREG(before.st_mode))return 0;
        NSString *start=ProcessStart(pid);if(!start)return 0;
        BOOL found=OwnerMapsImage(pid,&before);
        if(fstat(imageFD,&after)||![FileStamp(&before) isEqualToDictionary:FileStamp(&after)]
            ||![ProcessStart(pid) isEqualToString:start])return 0;
        return found?1:0;
    }
}
static NSDictionary *OwnerFilterFacts(pid_t pid, NSString *expectedStart, int imageFD, NSString *imagePath, NSString *bottle, NSString **reason) {
    *reason=@"owner_identity";
    if (![ProcessStart(pid) isEqualToString:expectedStart]) return nil;
    *reason=@"target_file_identity";
    struct stat image, current;
    if (fstat(imageFD,&image)||stat(imagePath.fileSystemRepresentation,&current)||!S_ISREG(image.st_mode)
        ||image.st_dev!=current.st_dev||image.st_ino!=current.st_ino)return nil;
    *reason=@"bottle_identity";
    NSDictionary *bottleIdentity=BottleIdentity(bottle);if(!bottleIdentity)return nil;
    *reason=@"owner_image_mapping";
    if(!OwnerMapsImage(pid,&image))return nil;
    *reason=@"owner_environment";
    BOOL envValid=NO;NSString *prefix=OwnerPrefix(pid,&envValid);if(!envValid)return nil;
    NSString *canonicalBottle=bottleIdentity[@"canonicalRoot"];
    if(prefix){if(![BottleIdentity(prefix)isEqualToDictionary:bottleIdentity])return nil;
        *reason=@"bottle_identity_changed";if(![BottleIdentity(bottle)isEqualToDictionary:bottleIdentity])return nil;
        return @{@"kind":@"owner_prefix_consistent",@"image":FileStamp(&image),@"bottle":bottleIdentity};}
    *reason=@"helper_running_bundle";
    char processPath[PROC_PIDPATHINFO_MAXSIZE];if(proc_pidpath(pid,processPath,sizeof(processPath))<=0)return nil;
    NSString *executable=[[NSString stringWithUTF8String:processPath] stringByResolvingSymlinksInPath];
    NSString *contents=executable.stringByDeletingLastPathComponent.stringByDeletingLastPathComponent;
    NSString *bundle=contents.stringByDeletingLastPathComponent;
    if(![contents.lastPathComponent isEqualToString:@"Contents"]||![bundle.pathExtension isEqualToString:@"app"])return nil;
    NSRunningApplication *application=[NSRunningApplication runningApplicationWithProcessIdentifier:pid];
    if(![application.bundleURL.path.stringByResolvingSymlinksInPath isEqualToString:bundle])return nil;
    *reason=@"helper_plist";
    NSDictionary *plistStamp=nil,*exeStamp=nil,*confStamp=nil;
    NSData *plistData=StableRead([contents stringByAppendingPathComponent:@"Info.plist"],1048576,&plistStamp);
    NSDictionary *plist=UniqueHelperPlist(plistData); if(!plist)return nil;
    NSString *bundleID=plist[@"CFBundleIdentifier"],*name=plist[@"CXHelperAppBottleName"],*tag=plist[@"CXHelperAppBottleTag"],*binary=plist[@"CFBundleExecutable"];
    *reason=@"helper_bundle_fields";
    if(![bundleID isKindOfClass:NSString.class]||![name isKindOfClass:NSString.class]||![tag isKindOfClass:NSString.class]||![binary isKindOfClass:NSString.class]||![binary isEqualToString:@"Menu Helper"]
        ||![bundleID hasPrefix:@"com.codeweavers.CrossOverHelper."]||![application.bundleIdentifier isEqualToString:bundleID]
        ||![[contents stringByAppendingPathComponent:[@"MacOS/" stringByAppendingString:binary]].stringByResolvingSymlinksInPath isEqualToString:executable]
        ||![name isEqualToString:canonicalBottle.lastPathComponent])return nil;
    if(!StableRead(executable,33554432,&exeStamp))return nil;
    *reason=@"helper_modified_after_start";
    long long started=expectedStart.longLongValue;
    NSArray *startParts=[expectedStart componentsSeparatedByString:@":"];long long startedNano=startParts.count==2?[startParts[1] longLongValue]*1000:0;
    for(NSDictionary *stamp in @[plistStamp,exeStamp]){long long sec=[stamp[@"mtimeSec"] longLongValue];if(sec>started||(sec==started&&[stamp[@"mtimeNano"] longLongValue]>startedNano))return nil;}
    *reason=@"bottle_configuration";
    NSData *conf=StableRead([canonicalBottle stringByAppendingPathComponent:@"cxbottle.conf"],1048576,&confStamp);
    NSString *uuid=ConfBottleID(conf);if(!uuid||![tag isEqualToString:[NSString stringWithFormat:@"CrossOver-%@/",uuid]])return nil;
    *reason=@"bottle_id_inventory";
    NSString *searchRoot=canonicalBottle.stringByDeletingLastPathComponent;
    NSArray *entries=[NSFileManager.defaultManager contentsOfDirectoryAtPath:searchRoot error:NULL];if(!entries||entries.count>256)return nil;
    unsigned matches=0;
    for(NSString *entry in entries){NSString *dir=[searchRoot stringByAppendingPathComponent:entry];BOOL isDir=NO;if(![NSFileManager.defaultManager fileExistsAtPath:dir isDirectory:&isDir]||!isDir)continue;
        NSString *confPath=[dir stringByAppendingPathComponent:@"cxbottle.conf"];if(![NSFileManager.defaultManager fileExistsAtPath:confPath])continue;
        NSDictionary *unused=nil;NSData *other=StableRead(confPath,1048576,&unused);if(!other)return nil;NSString *otherID=ConfBottleID(other);if(!otherID)return nil;if([otherID isEqualToString:uuid])matches++;
    }
    if(matches!=1||![ProcessStart(pid) isEqualToString:expectedStart])return nil;
    *reason=@"bottle_identity_changed";if(![BottleIdentity(bottle)isEqualToDictionary:bottleIdentity])return nil;
    return @{@"kind":@"helper_metadata_consistent",@"image":FileStamp(&image),@"bottle":bottleIdentity,@"helper":exeStamp,@"plist":plistStamp,@"conf":confStamp};
}
char *luna_preview_selection_validate(const char *transaction,int imageFD,const char *imagePath,const char *bottle) {
    @autoreleasepool {
        if(!CGPreflightScreenCaptureAccess())return JSON(@{@"error":@"preview_permission_revoked"});
        if(!transaction||!imagePath||!bottle)return JSON(@{@"error":@"preview_selection_mismatch"});
        LunaPickedWindow *record=PickedLookup([NSString stringWithUTF8String:transaction]);if(!record)return JSON(@{@"error":@"preview_selection_expired"});
        NSDictionary *identity=record.identity;uint32_t windowID=[identity[@"windowId"] unsignedIntValue];pid_t pid=[identity[@"hostPid"] intValue];NSString *start=identity[@"hostProcessStart"];
        if(!luna_preview_identity_current(windowID,pid,start.UTF8String))return JSON(@{@"error":@"preview_target_closed"});
        NSString *reason=nil;
        NSDictionary *facts=OwnerFilterFacts(pid,start,imageFD,[NSString stringWithUTF8String:imagePath],[NSString stringWithUTF8String:bottle],&reason);
        if(!facts)return JSON(@{@"error":@"preview_selection_mismatch",@"reason":reason?:@"unknown"});
        if(!CGPreflightScreenCaptureAccess())return JSON(@{@"error":@"preview_permission_revoked"});
        if(!luna_preview_identity_current(windowID,pid,start.UTF8String))return JSON(@{@"error":@"preview_target_closed"});
        struct stat heldImage,currentImage;
        if(fstat(imageFD,&heldImage)||stat(imagePath,&currentImage)
           ||![FileStamp(&heldImage) isEqualToDictionary:facts[@"image"]]
           ||![FileStamp(&currentImage) isEqualToDictionary:facts[@"image"]])
            return JSON(@{@"error":@"preview_selection_mismatch",@"reason":@"target_file_changed"});
        if(!AtLeast(15,2)||record.filter.style!=SCShareableContentStyleWindow||record.filter.includedWindows.count!=1)
            return JSON(@{@"error":@"preview_selection_mismatch",@"reason":@"filter_scope"});
        SCWindow *only=record.filter.includedWindows.firstObject;
        if(only.windowID!=windowID||only.owningApplication.processID!=pid)
            return JSON(@{@"error":@"preview_selection_mismatch",@"reason":@"filter_identity"});
        [record.lock lock];BOOL same=!record.evidence||[record.evidence isEqualToDictionary:facts];if(same)record.evidence=facts;[record.lock unlock];
        return JSON(same?@{@"kind":facts[@"kind"]}:@{@"error":@"preview_selection_mismatch",@"reason":@"owner_evidence_changed"});
    }
}

int luna_cover_normalize(const unsigned char *input, size_t inputLength, unsigned char **bytes, size_t *length, uint32_t *width, uint32_t *height) {
    @autoreleasepool {
        uint32_t originalWidth = 0, originalHeight = 0;
        CGImageSourceRef source = CoverSource(input, inputLength, &originalWidth, &originalHeight);
        if (!source) return 1;
        CGImageRef image = CGImageSourceCreateThumbnailAtIndex(source, 0, (__bridge CFDictionaryRef)@{
            (id)kCGImageSourceCreateThumbnailFromImageAlways: @YES,
            (id)kCGImageSourceCreateThumbnailWithTransform: @YES,
            (id)kCGImageSourceThumbnailMaxPixelSize: @960,
            (id)kCGImageSourceShouldCacheImmediately: @YES
        });
        CFRelease(source);
        if (!image) return 1;
        uint32_t w = (uint32_t)CGImageGetWidth(image), h = (uint32_t)CGImageGetHeight(image);
        if (w < 160 || h < 90 || w > 960 || h > 960) { CGImageRelease(image); return 2; }
        NSMutableData *encoded = [NSMutableData data];
        CGImageDestinationRef destination = CGImageDestinationCreateWithData((__bridge CFMutableDataRef)encoded, CFSTR("public.jpeg"), 1, NULL);
        if (!destination) { CGImageRelease(image); return 1; }
        CGImageDestinationAddImage(destination, image, (__bridge CFDictionaryRef)@{(id)kCGImageDestinationLossyCompressionQuality: @0.7});
        bool ok = CGImageDestinationFinalize(destination);
        CFRelease(destination); CGImageRelease(image);
        if (!ok || !encoded.length || encoded.length > 524288) return 2;
        *bytes = malloc(encoded.length); if (!*bytes) return 2;
        memcpy(*bytes, encoded.bytes, encoded.length); *length = encoded.length; *width = w; *height = h;
        return 0;
    }
}
static atomic_bool captureBusy = false;
int luna_capture_busy(void) { return atomic_load(&captureBusy); }
static dispatch_queue_t PreviewWorkQueue(void) {
    static dispatch_queue_t queue;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ queue = dispatch_queue_create("luna.preview.work", DISPATCH_QUEUE_SERIAL); });
    return queue;
}
static BOOL FinitePositive(double value) { return isfinite(value) && value > 0; }
static BOOL GeometryPixel(double points, double scale, uint64_t *pixel) {
    if (!FinitePositive(points) || !FinitePositive(scale)) return NO;
    double value = ceil(points * scale);
    if (!isfinite(value) || value < 1 || value >= 18446744073709551616.0) return NO;
    *pixel = (uint64_t)value; return YES;
}
// The actual request uses filter points and its own pixel scale. Source bounds
// remain a separate conservative CLI gate; a >4MP source can still request a
// <=960x540 ScreenCaptureKit image.
static NSString *GeometryRequest(double width, double height, double scale, uint32_t *outWidth, uint32_t *outHeight) {
    if (!FinitePositive(width) || !FinitePositive(height)) return @"filter_dimensions";
    if (!FinitePositive(scale)) return @"point_pixel_scale";
    uint64_t checkedWidth, checkedHeight;
    if (!GeometryPixel(width, scale, &checkedWidth) || !GeometryPixel(height, scale, &checkedHeight)) return @"pixel_overflow";
    double sourceWidth = width * scale, sourceHeight = height * scale;
    double ratio = MIN(1.0, MIN(960.0 / sourceWidth, 540.0 / sourceHeight));
    double w = MAX(1.0, floor(sourceWidth * ratio)), h = MAX(1.0, floor(sourceHeight * ratio));
    if (!isfinite(w) || !isfinite(h) || w > 960 || h > 540) return @"pixel_overflow";
    *outWidth = (uint32_t)w; *outHeight = (uint32_t)h; return nil;
}
int luna_preview_geometry_request_for_test(double width, double height, double scale, uint32_t *outWidth, uint32_t *outHeight) {
    return GeometryRequest(width, height, scale, outWidth, outHeight) ? 1 : 0;
}
static id FinitePoint(double value) { return FinitePositive(value) ? @(value) : NSNull.null; }
static NSDictionary *FilterGeometry(SCContentFilter *filter, uint32_t windowId, int hostPid, SCWindow *queriedWindow) {
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, windowId));
    NSDictionary *exact = nil;
    for (NSDictionary *window in windows)
        if ([window[(id)kCGWindowNumber] unsignedIntValue] == windowId && [window[(id)kCGWindowOwnerPID] intValue] == hostPid) { exact = window; break; }
    CGRect bounds = CGRectZero;
    id boundsValue = exact[(id)kCGWindowBounds];
    BOOL hasBounds = [boundsValue isKindOfClass:NSDictionary.class] && CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)boundsValue, &bounds);
    CGRect content = filter.contentRect;
    double scale = filter.pointPixelScale;
    uint32_t width = 0, height = 0;
    NSString *reason = GeometryRequest(content.size.width, content.size.height, scale, &width, &height);
    uint64_t cgW = 0, cgH = 0, filterW = 0, filterH = 0;
    if (!reason && (!hasBounds || !exact[(id)kCGWindowIsOnscreen])) reason = @"cg_missing";
    if (!reason && (!FinitePositive(bounds.size.width) || !FinitePositive(bounds.size.height))) reason = @"cg_dimensions";
    if (!reason && (!GeometryPixel(bounds.size.width, scale, &cgW) || !GeometryPixel(bounds.size.height, scale, &cgH)
        || !GeometryPixel(content.size.width, scale, &filterW) || !GeometryPixel(content.size.height, scale, &filterH))) reason = @"pixel_overflow";
    BOOL sourceCap = NO;
    if (!reason) {
        uint64_t w = MAX(cgW, filterW), h = MAX(cgH, filterH);
        sourceCap = w > 4194304 || h > 4194304 || w > 4194304 / h;
    }
    // No historical SCWindow.frame is substituted for a metadata observation.
    return @{@"cg_width_points":hasBounds ? FinitePoint(bounds.size.width) : NSNull.null,
        @"cg_height_points":hasBounds ? FinitePoint(bounds.size.height) : NSNull.null,
        @"cg_on_screen":exact[(id)kCGWindowIsOnscreen] ?: NSNull.null,
        @"filter_width_points":FinitePoint(content.size.width), @"filter_height_points":FinitePoint(content.size.height),
        @"point_pixel_scale":FinitePoint(scale), @"request_width":reason ? NSNull.null : @(width),
        @"request_height":reason ? NSNull.null : @(height), @"compatibility_source_cap":reason ? NSNull.null : @(sourceCap),
        @"reason":reason ?: NSNull.null, @"sc_window_frame_query":queriedWindow ? @"queried" : @"not_queried",
        @"sc_window_width_points":queriedWindow ? FinitePoint(queriedWindow.frame.size.width) : NSNull.null,
        @"sc_window_height_points":queriedWindow ? FinitePoint(queriedWindow.frame.size.height) : NSNull.null};
}
static int RetainedFilterStatus(NSString *token, SCContentFilter *filter, uint32_t window, int pid, NSString *start) {
    if (!CGPreflightScreenCaptureAccess()) return 2;
    if (!AtLeast(15, 2)) return 1;
    LunaPickedWindow *record = PickedLookup(token);
    if (!record) return 10;
    if (record.filter != filter || [record.identity[@"windowId"] unsignedIntValue] != window
        || [record.identity[@"hostPid"] intValue] != pid || ![record.identity[@"hostProcessStart"] isEqualToString:start]
        || !luna_preview_identity_current(window, pid, start.UTF8String)) return 3;
    if (filter.style != SCShareableContentStyleWindow || filter.includedWindows.count != 1) return 3;
    SCWindow *only = filter.includedWindows.firstObject;
    return only.windowID == window && only.owningApplication.processID == pid ? 0 : 3;
}
@interface LunaGeometryResult : NSObject
@property NSDictionary *value;
@end
@implementation LunaGeometryResult
@end
static NSDictionary *RunGeometryWork(NSDictionary *(^read)(void), int64_t waitNanoseconds) {
    bool expected = false;
    if (!atomic_compare_exchange_strong(&captureBusy, &expected, true)) return @{@"error":@"preview_capture_busy"};
    LunaGeometryResult *result = [LunaGeometryResult new];
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    // One fixed serial channel and one heap result. A timeout leaves this
    // work owning the global gate until it really completes; no Rust stack
    // pointer, extra timeout worker, image, picker or content enumeration.
    dispatch_async(PreviewWorkQueue(), ^{
        @autoreleasepool {
            @try { result.value = read(); }
            @catch (NSException *exception) { (void)exception; result.value = @{@"error":@"preview_geometry_unavailable"}; }
            @finally { atomic_store(&captureBusy, false); dispatch_semaphore_signal(done); }
        }
    });
    if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, waitNanoseconds))) return @{@"error":@"preview_capture_timeout"};
    return result.value;
}
char *luna_preview_selection_geometry(const char *transaction, uint32_t window, int pid, const char *hostStart) {
    @autoreleasepool {
        if (!transaction || !hostStart) return JSON(@{@"error":@"preview_selection_expired"});
        NSString *token = [NSString stringWithUTF8String:transaction], *start = [NSString stringWithUTF8String:hostStart];
        LunaPickedWindow *record = PickedLookup(token);
        return JSON(RunGeometryWork(^NSDictionary *{
            int before = RetainedFilterStatus(token, record.filter, window, pid, start);
            NSDictionary *value = before ? nil : FilterGeometry(record.filter, window, pid, nil);
            int after = RetainedFilterStatus(token, record.filter, window, pid, start);
            int status = after ?: before;
            return status ? @{@"error":status == 2 ? @"preview_permission_revoked" : status == 3 ? @"preview_target_closed" : @"preview_selection_expired"} : value;
        }, 5 * NSEC_PER_SEC));
    }
}
// Test-only Rust declaration. This calls the real production dispatch/gate/wait
// helper with a controlled clock deadline and metadata-free read blocks.
int luna_preview_geometry_gate_for_test(void) {
    @autoreleasepool {
        __block NSUInteger calls = 0, active = 0, maximum = 0;
        dispatch_semaphore_t started = dispatch_semaphore_create(0), release = dispatch_semaphore_create(0), finished = dispatch_semaphore_create(0);
        NSDictionary *first = RunGeometryWork(^NSDictionary *{
            calls++; active++; maximum = MAX(maximum, active);
            dispatch_semaphore_signal(started);
            dispatch_semaphore_wait(release, DISPATCH_TIME_FOREVER);
            active--; dispatch_semaphore_signal(finished); return @{@"done":@YES};
        }, 0);
        int mask = [first[@"error"] isEqualToString:@"preview_capture_timeout"] ? 1 : 0;
        BOOL began = dispatch_semaphore_wait(started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0;
        NSDictionary *second = RunGeometryWork(^NSDictionary *{ calls++; return @{@"done":@YES}; }, 0);
        if ([second[@"error"] isEqualToString:@"preview_capture_busy"]) mask |= 2;
        if (began && luna_capture_busy() && calls == 1) mask |= 4;
        dispatch_semaphore_signal(release);
        BOOL ended = dispatch_semaphore_wait(finished, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0;
        // The controlled read has no OS calls and has already been released.
        // Drain the same serial channel before reading its completion facts.
        dispatch_sync(PreviewWorkQueue(), ^{});
        if (ended && !luna_capture_busy()) mask |= 8;
        NSDictionary *third = RunGeometryWork(^NSDictionary *{
            calls++; active++; maximum = MAX(maximum, active); active--; return @{@"done":@YES};
        }, NSEC_PER_SEC);
        if ([third[@"done"] boolValue]) mask |= 16;
        dispatch_sync(PreviewWorkQueue(), ^{});
        if (calls == 2) mask |= 32;
        if (maximum == 1) mask |= 64;
        return mask;
    }
}
static BOOL CaptureImageDimensionsAllowed(uint64_t width, uint64_t height) {
    return width > 0 && height > 0 && width <= 960 && height <= 540;
}
static NSString *CaptureBudgetReason(BOOL finalized, uint64_t width, uint64_t height, uint64_t encoded) {
    if (!finalized) return @"jpeg_finalize";
    if (!CaptureImageDimensionsAllowed(width, height)) return @"image_dimensions";
    if (!encoded || encoded > 524288) return @"jpeg_bytes";
    return nil;
}
int luna_preview_capture_budget_for_test(int finalized, uint64_t width, uint64_t height, uint64_t encoded) {
    if (!CaptureImageDimensionsAllowed(width, height)) return 1;
    return CaptureBudgetReason(finalized, width, height, encoded) ? 2 : 0;
}
static int CaptureDiagnosticReturn(int status, unsigned long long operation, uint32_t window, int pid, const char *start, LunaCaptureResult *result, NSString *reason) {
    NSMutableDictionary *value = [@{@"stage": status == 8 ? @"capture_timeout" : @"capture_complete", @"operation":@(operation), @"revision":@(operation), @"native_status":@(status)} mutableCopy];
    // A timed-out callback can finish later; it never publishes this snapshot
    // and we do not read its still-mutating result from the timeout path.
    if (status != 8 && result) {
        value[@"configured_width"] = @(result.configuredWidth); value[@"configured_height"] = @(result.configuredHeight);
        value[@"actual_width"] = @(result.width); value[@"actual_height"] = @(result.height);
        value[@"encoded_bytes"] = @(result.encodedBytes);
        if (result.scDomain) { value[@"sc_domain"] = result.scDomain; value[@"sc_code"] = @(result.scCode); }
        if (result.api) value[@"api"] = result.api;
        if (result.geometry) value[@"geometry"] = result.geometry;
        if (result.failureKind) value[@"failure_kind"] = result.failureKind;
        if (!reason) reason = result.reason ?: result.error;
    }
    if (reason) value[@"reason"] = reason;
    PickedInitialize(); NSDictionary *identity = @{@"window":@(window), @"pid":@(pid), @"start":start ? [NSString stringWithUTF8String:start] ?: @"" : @""};
    [PickedLock lock]; LastCaptureDiagnostic = [value copy]; LastCaptureDiagnosticIdentity = identity; [PickedLock unlock];
    return status;
}
// Called only by the preview worker. All callbacks retain their own result;
// timeouts never leave an asynchronous callback pointing at a Rust stack frame.
// The original picker capability is already restricted to this exact single
// window. A wake/Space transition can make its metadata temporarily offscreen;
// let ScreenCaptureKit decide whether that retained filter can produce a frame.
// Automatic windows have no retained capability and retain the on-screen gate.
int luna_preview_capture_on_screen_allowed(int preserved, int onScreen) {
    return preserved || onScreen;
}
// Production policy: a revocation already observed for this exact retained
// capability stays invalid even if a later permission query reports a grant.
// This returns a fixed status only; it never changes or creates a capability.
int luna_preview_capture_post_status(int retainedStatus, int processCurrent, int permissionGranted) {
    if (retainedStatus == 2 || !permissionGranted) return 2;
    if (retainedStatus == 10) return 10;
    if (retainedStatus != 0 || !processCurrent) return 3;
    return 0;
}

static int CaptureWindow(SCContentFilter *preservedFilter, NSString *selectionToken, uint32_t windowId, int hostPid, const char *hostStart,
                        unsigned char **bytes, size_t *length, uint32_t *width,
                        uint32_t *height, uint64_t *capturedAt) {
    @autoreleasepool {
        unsigned long long operation = atomic_fetch_add(&DiagnosticSequence, 1) + 1;
        LunaCaptureResult *result = nil;
        if (AtLeast(14, 0)) {
            NSString *expectedStart = [NSString stringWithUTF8String:hostStart];
            bool expected = false;
            if (!atomic_compare_exchange_strong(&captureBusy, &expected, true)) return CaptureDiagnosticReturn(7,operation,windowId,hostPid,hostStart,nil,@"capture_busy");
            dispatch_semaphore_t done = dispatch_semaphore_create(0);
            result = [LunaCaptureResult new];
            result.api = @"shareable_content";
            dispatch_async(PreviewWorkQueue(), ^{
            int preStatus = preservedFilter ? RetainedFilterStatus(selectionToken, preservedFilter, windowId, hostPid, expectedStart) : 0;
            if (preStatus || !CGPreflightScreenCaptureAccess() || ![ProcessStart(hostPid) isEqualToString:expectedStart]) {
                result.error = preStatus == 10 ? @"selection_expired" : preStatus == 2 || !CGPreflightScreenCaptureAccess() ? @"permission_revoked" : @"target_closed";
                atomic_store(&captureBusy, false); dispatch_semaphore_signal(done); return;
            }
            [SCShareableContent getShareableContentExcludingDesktopWindows:YES onScreenWindowsOnly:NO completionHandler:^(SCShareableContent *content, NSError *error) {
                @autoreleasepool {
                    SCWindow *selected = nil;
                    if (error) { result.scDomain = [error.domain isEqualToString:SCStreamErrorDomain] ? @"sc_stream" : @"other"; result.scCode = error.code; }
                    for (SCWindow *window in content.windows) {
                        if (window.windowID == windowId && window.owningApplication.processID == hostPid) { selected = window; break; }
                    }
                    if (!selected || error || ![ProcessStart(hostPid) isEqualToString:expectedStart]) {
                        result.error = @"target_closed"; atomic_store(&captureBusy, false); dispatch_semaphore_signal(done); return;
                    }
                    if (!luna_preview_capture_on_screen_allowed(preservedFilter != nil, selected.isOnScreen)) {
                        result.error = @"window_not_on_screen"; atomic_store(&captureBusy, false); dispatch_semaphore_signal(done); return;
                    }
                    SCContentFilter *filter = preservedFilter ?: [[SCContentFilter alloc] initWithDesktopIndependentWindow:selected];
                    int retainedStatus = preservedFilter ? RetainedFilterStatus(selectionToken, preservedFilter, windowId, hostPid, expectedStart) : 0;
                    if (retainedStatus) {
                        result.error = retainedStatus == 2 ? @"permission_revoked" : retainedStatus == 10 ? @"selection_expired" : @"target_closed";
                        atomic_store(&captureBusy, false); dispatch_semaphore_signal(done); return;
                    }
                    // This same-attempt snapshot is obtained before changing
                    // the request, and the exact values form the request below.
                    result.geometry = FilterGeometry(filter, windowId, hostPid, selected);
                    if (result.geometry[@"reason"] != NSNull.null) {
                        result.error = @"geometry_unavailable"; result.reason = @"geometry_unavailable";
                        result.failureKind = @"geometry_unavailable";
                        atomic_store(&captureBusy, false); dispatch_semaphore_signal(done); return;
                    }
                    SCStreamConfiguration *config = [SCStreamConfiguration new];
                    config.width = [result.geometry[@"request_width"] unsignedIntValue];
                    config.height = [result.geometry[@"request_height"] unsignedIntValue];
                    result.configuredWidth = (uint32_t)config.width; result.configuredHeight = (uint32_t)config.height;
                    config.showsCursor = NO;
                    config.capturesAudio = NO;
                    if (AtLeast(14, 2)) config.includeChildWindows = NO;
                    result.api = @"screenshot";
                    [SCScreenshotManager captureImageWithFilter:filter configuration:config completionHandler:^(CGImageRef image, NSError *captureError) {
                        @autoreleasepool {
                            if (captureError) { result.scDomain = [captureError.domain isEqualToString:SCStreamErrorDomain] ? @"sc_stream" : @"other"; result.scCode = captureError.code; }
                            int retainedStatus = preservedFilter ? RetainedFilterStatus(selectionToken, preservedFilter, windowId, hostPid, expectedStart) : 0;
                            BOOL processCurrent = [ProcessStart(hostPid) isEqualToString:expectedStart];
                            BOOL permissionGranted = CGPreflightScreenCaptureAccess();
                            int postStatus = luna_preview_capture_post_status(retainedStatus, processCurrent, permissionGranted);
                            if (postStatus == 2) result.error = @"permission_revoked";
                            else if (postStatus == 10) result.error = @"selection_expired";
                            else if (postStatus == 3) result.error = @"target_closed";
                            else if (CaptureFailureCode(captureError)==10) result.error = @"selection_expired";
                            else if (CaptureFailureCode(captureError)==9) result.error = @"sc_compatibility_failed";
                            else if (!image || captureError) { result.error = @"capture_failed"; result.reason = captureError ? @"sc_error" : @"image_missing"; }
                            else {
                                // Timestamp belongs to this completed native capture, never a UI heartbeat.
                                result.capturedAt = (uint64_t)(NSDate.date.timeIntervalSince1970 * 1000.0);
                                result.width = CGImageGetWidth(image); result.height = CGImageGetHeight(image);
                                if (!CaptureImageDimensionsAllowed(result.width, result.height)) {
                                    // Reject the real CGImage before creating a
                                    // JPEG destination or encoding any pixels.
                                    result.error = @"preview_budget_exceeded"; result.reason = @"image_dimensions";
                                    result.failureKind = @"output_safety_budget";
                                    atomic_store(&captureBusy, false); dispatch_semaphore_signal(done); return;
                                }
                                NSMutableData *encoded = [NSMutableData data];
                                result.api = @"jpeg";
                                CGImageDestinationRef destination = CGImageDestinationCreateWithData((__bridge CFMutableDataRef)encoded, CFSTR("public.jpeg"), 1, NULL);
                                if (destination) {
                                    CGImageDestinationAddImage(destination, image, (__bridge CFDictionaryRef)@{(id)kCGImageDestinationLossyCompressionQuality: @0.65});
                                    BOOL finalized = CGImageDestinationFinalize(destination);
                                    result.encodedBytes = encoded.length;
                                    NSString *budgetReason = CaptureBudgetReason(finalized, result.width, result.height, encoded.length);
                                    if (!budgetReason) result.data = encoded;
                                    else { result.error = @"preview_budget_exceeded"; result.reason = budgetReason; result.failureKind = @"output_safety_budget"; }
                                    CFRelease(destination);
                                } else { result.error = @"preview_encoding_failed"; result.reason = @"jpeg_destination"; }
                            }
                            atomic_store(&captureBusy, false); dispatch_semaphore_signal(done);
                        }
                    }];
                }
            }];
            });
            if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC))) return CaptureDiagnosticReturn(8,operation,windowId,hostPid,hostStart,nil,@"capture_timeout");
            if (!result.data || result.error) {
                int status = [result.error isEqualToString:@"permission_revoked"] ? 2 : [result.error isEqualToString:@"target_closed"] ? 3 : [result.error isEqualToString:@"window_not_on_screen"] ? 4 : [result.error isEqualToString:@"preview_budget_exceeded"] ? 5 : [result.error isEqualToString:@"sc_compatibility_failed"] ? 9 : [result.error isEqualToString:@"selection_expired"] ? 10 : 6;
                return CaptureDiagnosticReturn(status,operation,windowId,hostPid,hostStart,result,nil);
            }
            *length = result.data.length; *bytes = malloc(*length);
            if (!*bytes) { result.failureKind = @"output_safety_budget"; return CaptureDiagnosticReturn(5,operation,windowId,hostPid,hostStart,result,@"allocation_failed"); }
            memcpy(*bytes, result.data.bytes, *length);
            *width = result.width; *height = result.height; *capturedAt = result.capturedAt;
            return CaptureDiagnosticReturn(0,operation,windowId,hostPid,hostStart,result,nil);
        }
        return CaptureDiagnosticReturn(1,operation,windowId,hostPid,hostStart,nil,nil);
    }
}

int luna_capture_window(uint32_t windowId,int hostPid,const char *hostStart,unsigned char **bytes,size_t *length,uint32_t *width,uint32_t *height,uint64_t *capturedAt) {
    return CaptureWindow(nil,nil,windowId,hostPid,hostStart,bytes,length,width,height,capturedAt);
}
int luna_capture_selected_window(const char *transaction,uint32_t windowId,int hostPid,const char *hostStart,unsigned char **bytes,size_t *length,uint32_t *width,uint32_t *height,uint64_t *capturedAt) {
    if(!transaction)return 3;
    LunaPickedWindow *record=PickedLookup([NSString stringWithUTF8String:transaction]);
    if(!record || [record.identity[@"windowId"] unsignedIntValue]!=windowId || [record.identity[@"hostPid"] intValue]!=hostPid
        || ![record.identity[@"hostProcessStart"] isEqualToString:[NSString stringWithUTF8String:hostStart]])return 3;
    if (!AtLeast(15, 2)) return 1;
    // Window style/single included owner/start validation runs on the same
    // bounded work channel before enumeration and again before/after capture.
    return CaptureWindow(record.filter,[NSString stringWithUTF8String:transaction],windowId,hostPid,hostStart,bytes,length,width,height,capturedAt);
}
// A retained single-window picker filter is mandatory for the compatibility
// adapter. Its source budget is checked before spawning Apple's window-only
// capture tool; this function never captures a desktop or changes a window.
int luna_preview_selection_capture_bounds(const char *transaction,uint32_t windowId,int hostPid,const char *hostStart,uint32_t *width,uint32_t *height) {
    @autoreleasepool {
        if(!width||!height)return 1;
        char *raw=luna_preview_selection_geometry(transaction,windowId,hostPid,hostStart);
        if(!raw)return 6;
        NSData *data=[NSData dataWithBytes:raw length:strlen(raw)]; free(raw);
        NSDictionary *geometry=[NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
        NSString *failure=geometry[@"error"];
        if(failure)return [failure isEqualToString:@"preview_permission_revoked"]?2:
            [failure isEqualToString:@"preview_target_closed"]?3:
            [failure isEqualToString:@"preview_capture_busy"]?7:
            [failure isEqualToString:@"preview_capture_timeout"]?8:10;
        if(!geometry||geometry[@"reason"]!=NSNull.null)return 5;
        uint64_t cgW,cgH,filterW,filterH;
        double scale=[geometry[@"point_pixel_scale"] doubleValue];
        if(!GeometryPixel([geometry[@"cg_width_points"] doubleValue],scale,&cgW)
           ||!GeometryPixel([geometry[@"cg_height_points"] doubleValue],scale,&cgH)
           ||!GeometryPixel([geometry[@"filter_width_points"] doubleValue],scale,&filterW)
           ||!GeometryPixel([geometry[@"filter_height_points"] doubleValue],scale,&filterH))return 5;
        uint64_t w=MAX(cgW,filterW),h=MAX(cgH,filterH);
        if(w>4194304||h>4194304||w>4194304/h)return 5;
        *width=(uint32_t)w;*height=(uint32_t)h;
        return 0;
    }
}
int luna_preview_normalize(const unsigned char *input,size_t inputLength,unsigned char **bytes,size_t *length,uint32_t *width,uint32_t *height) {
    @autoreleasepool {
        if(!input||inputLength>2097152)return 2;
        uint32_t sourceWidth=0,sourceHeight=0;
        CGImageSourceRef source=CoverSource(input,inputLength,&sourceWidth,&sourceHeight);
        if(!source)return 1;
        CFStringRef type=CGImageSourceGetType(source);
        if(!type||!CFEqual(type,CFSTR("public.jpeg"))||(uint64_t)sourceWidth*sourceHeight>4194304){CFRelease(source);return 2;}
        double ratio=MIN(1.0,MIN(960.0/sourceWidth,540.0/sourceHeight));
        NSUInteger limit=MAX(1,floor(MAX(sourceWidth,sourceHeight)*ratio));
        CGImageRef image=CGImageSourceCreateThumbnailAtIndex(source,0,(__bridge CFDictionaryRef)@{
            (id)kCGImageSourceCreateThumbnailFromImageAlways:@YES,
            (id)kCGImageSourceCreateThumbnailWithTransform:@YES,
            (id)kCGImageSourceThumbnailMaxPixelSize:@(limit)});
        CFRelease(source);if(!image)return 1;
        size_t w=CGImageGetWidth(image),h=CGImageGetHeight(image);
        if(!w||!h||w>960||h>540){CGImageRelease(image);return 2;}
        NSMutableData *encoded=[NSMutableData data];
        CGImageDestinationRef destination=CGImageDestinationCreateWithData((__bridge CFMutableDataRef)encoded,CFSTR("public.jpeg"),1,NULL);
        if(!destination){CGImageRelease(image);return 1;}
        CGImageDestinationAddImage(destination,image,(__bridge CFDictionaryRef)@{(id)kCGImageDestinationLossyCompressionQuality:@0.65});
        BOOL valid=CGImageDestinationFinalize(destination)&&encoded.length>0&&encoded.length<=524288;
        CFRelease(destination);CGImageRelease(image);if(!valid)return 2;
        *bytes=malloc(encoded.length);if(!*bytes)return 2;
        memcpy(*bytes,encoded.bytes,encoded.length);*length=encoded.length;*width=(uint32_t)w;*height=(uint32_t)h;
        return 0;
    }
}
typedef void (*LunaHotkeyEdge)(uint32_t, int);
static LunaHotkeyEdge edgeCallback = NULL;
static NSMutableDictionary<NSNumber *, NSValue *> *hotkeys;
static EventHandlerRef hotkeyHandler;
static OSStatus HotkeyEvent(EventHandlerCallRef next, EventRef event, void *context) {
    (void)next; (void)context;
    EventHotKeyID identity = {0};
    OSStatus status = GetEventParameter(event, kEventParamDirectObject, typeEventHotKeyID, NULL, sizeof(identity), NULL, &identity);
    if (status == noErr && identity.signature == 'Luna' && edgeCallback) edgeCallback(identity.id, GetEventKind(event) == kEventHotKeyPressed);
    return status;
}
int luna_hotkey_initialize(LunaHotkeyEdge callback) {
    __block OSStatus result = noErr;
    OnMain(^{
        edgeCallback = callback;
        if (!hotkeyHandler) {
            EventTypeSpec events[] = {{kEventClassKeyboard, kEventHotKeyPressed}, {kEventClassKeyboard, kEventHotKeyReleased}};
            result = InstallApplicationEventHandler(HotkeyEvent, 2, events, NULL, &hotkeyHandler);
            if (result == noErr) hotkeys = [NSMutableDictionary dictionary];
        }
    });
    return result;
}
int luna_hotkey_register(uint32_t id, uint32_t key, uint32_t modifiers) {
    __block OSStatus result = -1;
    OnMain(^{
        UInt32 flags = 0;
        if (modifiers & 1) flags |= cmdKey;
        if (modifiers & 2) flags |= optionKey;
        if (modifiers & 4) flags |= controlKey;
        if (modifiers & 8) flags |= shiftKey;
        EventHotKeyRef reference = NULL;
        result = RegisterEventHotKey(key, flags, (EventHotKeyID){'Luna', id}, GetApplicationEventTarget(), 0, &reference);
        if (result == noErr) hotkeys[@(id)] = [NSValue valueWithPointer:reference];
    });
    return result;
}
int luna_hotkey_unregister(uint32_t id) {
    __block OSStatus result = noErr;
    OnMain(^{
        NSValue *value = hotkeys[@(id)];
        if (value) { result = UnregisterEventHotKey(value.pointerValue); if (result == noErr) [hotkeys removeObjectForKey:@(id)]; }
    });
    return result;
}

// R39-A same-executable collector. These entry points never create Tauri,
// open profile storage, attach a Hook host, or inherit arbitrary descriptors.
#include <spawn.h>
#include <sys/socket.h>
#include <sys/wait.h>
static atomic_bool CollectorStop = false;
char *luna_collector_process_identity(int pid) {
    @autoreleasepool {
        NSString *start=ProcessStart(pid);char path[PROC_PIDPATHINFO_MAXSIZE];struct stat st;
        if(!start||proc_pidpath(pid,path,sizeof(path))<=0||stat(path,&st)||!S_ISREG(st.st_mode))return JSON(@{@"error":@"collector_process_identity"});
        if(![ProcessStart(pid)isEqualToString:start]||!OwnerMapsImage(pid,&st))return JSON(@{@"error":@"collector_process_identity"});
        struct stat after;if(stat(path,&after)||st.st_dev!=after.st_dev||st.st_ino!=after.st_ino||![ProcessStart(pid)isEqualToString:start])return JSON(@{@"error":@"collector_process_identity"});
        return JSON(@{@"pid":@(pid),@"start":start,@"dev":@((uint64_t)st.st_dev),@"ino":@((uint64_t)st.st_ino)});
    }
}
int luna_collector_private_fd(int fd) {
    int type=0;socklen_t length=sizeof(type);uid_t uid;gid_t gid;
    if(fd<3||getsockopt(fd,SOL_SOCKET,SO_TYPE,&type,&length)||type!=SOCK_STREAM||getpeereid(fd,&uid,&gid)||uid!=geteuid())return 0;
    int flags=fcntl(fd,F_GETFD);return flags>=0&&!fcntl(fd,F_SETFD,flags|FD_CLOEXEC);
}
int luna_collector_parent(int parent) {
    if(parent<=1||getppid()!=parent)return 0;
    char own[PROC_PIDPATHINFO_MAXSIZE],other[PROC_PIDPATHINFO_MAXSIZE];struct stat a,b;
    return proc_pidpath(getpid(),own,sizeof(own))>0&&proc_pidpath(parent,other,sizeof(other))>0
        &&!stat(own,&a)&&!stat(other,&b)&&S_ISREG(a.st_mode)&&S_ISREG(b.st_mode)&&a.st_dev==b.st_dev&&a.st_ino==b.st_ino&&OwnerMapsImage(getpid(),&a)&&OwnerMapsImage(parent,&b);
}
static int CollectorSpawn(const char *executable,int controlFD,int frameFD,int *pid,char *const *arguments) {
    if(!executable||executable[0]!='/'||!pid||controlFD==frameFD)return EINVAL;
    int control=fcntl(controlFD,F_DUPFD_CLOEXEC,5);if(control<0)return errno;
    int frame=fcntl(frameFD,F_DUPFD_CLOEXEC,5);if(frame<0){int e=errno;close(control);return e;}
    posix_spawn_file_actions_t actions;posix_spawnattr_t attr;
    int status=posix_spawn_file_actions_init(&actions);if(status){close(control);close(frame);return status;}
    status=posix_spawnattr_init(&attr);if(status){posix_spawn_file_actions_destroy(&actions);close(control);close(frame);return status;}
    if(!status)status=posix_spawnattr_setflags(&attr,POSIX_SPAWN_CLOEXEC_DEFAULT);
    if(!status)status=posix_spawn_file_actions_adddup2(&actions,control,3);
    if(!status)status=posix_spawn_file_actions_adddup2(&actions,frame,4);
    for(int fd=0;fd<3&&!status;fd++)status=posix_spawn_file_actions_addopen(&actions,fd,"/dev/null",fd?O_WRONLY:O_RDONLY,0);
    // Only non-secret platform locale/home facts; never inherit model tokens.
    char *environment[9]={0};unsigned count=0;const char *names[]={"HOME","USER","LOGNAME","TMPDIR","LANG","LC_CTYPE"};
    for(unsigned i=0;i<6;i++){const char *value=getenv(names[i]);if(value&&strlen(value)<=4096){if(asprintf(&environment[count],"%s=%s",names[i],value)<0){status=ENOMEM;break;}count++;}}
    environment[count++]=strdup("PATH=/usr/bin:/bin:/usr/sbin:/sbin");if(!environment[count-1])status=ENOMEM;
    pid_t child=0;
    if(!status)status=posix_spawn(&child,executable,&actions,&attr,arguments,environment);
    for(unsigned i=0;i<count;i++)free(environment[i]);posix_spawnattr_destroy(&attr);posix_spawn_file_actions_destroy(&actions);close(control);close(frame);
    if(!status)*pid=child;return status;
}
int luna_collector_spawn(const char *executable,int controlFD,int frameFD,int *pid) {
    char *arguments[]={(char *)executable,"--capture-worker","--control-fd","3","--frame-fd","4",NULL};
    return CollectorSpawn(executable,controlFD,frameFD,pid,arguments);
}
// Permanent test seam exercises the exact production file-actions/allowlist.
// Only fixed shell built-ins inspect integer FD numbers; never a user command.
int luna_collector_fd_probe_for_test(int controlFD,int frameFD,int leakedFD,int *pid) {
    if(leakedFD<5||leakedFD>1048576)return EINVAL;
    char script[256];snprintf(script,sizeof(script),"test -e /dev/fd/3 && test -e /dev/fd/4 && test ! -e /dev/fd/%d",leakedFD);
    char *arguments[]={"/bin/sh","-c",script,NULL};return CollectorSpawn("/bin/sh",controlFD,frameFD,pid,arguments);
}
int luna_collector_reap(int pid,int *exitCode) {
    int status=0;pid_t value=waitpid(pid,&status,WNOHANG);if(value==0)return 0;if(value<0)return -1;
    if(exitCode)*exitCode=WIFEXITED(status)?WEXITSTATUS(status):128+(WIFSIGNALED(status)?WTERMSIG(status):0);return 1;
}
int luna_collector_runloop(void) {
    if(!NSThread.isMainThread)return 1;
    @autoreleasepool {
        if(atomic_load(&CollectorStop))return 0;
        [NSApplication sharedApplication];[NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        if(!atomic_load(&CollectorStop))[NSApp run];return 0;
    }
}
void luna_collector_stop_runloop(void) {
    atomic_store(&CollectorStop,true);
    CFRunLoopPerformBlock(CFRunLoopGetMain(),kCFRunLoopCommonModes,^{
        [NSApp stop:nil];NSEvent *event=[NSEvent otherEventWithType:NSEventTypeApplicationDefined location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil subtype:0 data1:0 data2:0];if(event)[NSApp postEvent:event atStart:NO];
    });CFRunLoopWakeUp(CFRunLoopGetMain());
}
