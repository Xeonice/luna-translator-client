/* Owned, nonactivating offscreen WinAPI fixture. Never addresses another HWND.
 * The actual maintained Rust target_probe is the only external proof writer. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <bcrypt.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <stdint.h>

static HWND own_window;
static WCHAR own_class[160];
static WCHAR baseline[8192];
static char mode[48];
static unsigned long writes;
static int testing;
static const WCHAR normal_title[] = L"日本語メモリーズオフ 🌸 e\x0301 fixture";
static const WCHAR third_title[] = L"ThirdParty-日本語-caption";
static void hex(const void *p, size_t n) {
    const unsigned char *b = p;
    for (size_t i = 0; i < n; ++i) printf("%02x", b[i]);
}
static int image_sha256(const WCHAR *path, char result[65]) {
    BCRYPT_ALG_HANDLE alg=NULL; BCRYPT_HASH_HANDLE hash=NULL;
    DWORD size=0, count=0; unsigned char *object=NULL, bytes[65536], sum[32];
    HANDLE file=INVALID_HANDLE_VALUE; int ok=0;
    if(BCryptOpenAlgorithmProvider(&alg,BCRYPT_SHA256_ALGORITHM,NULL,0)<0)goto done;
    if(BCryptGetProperty(alg,BCRYPT_OBJECT_LENGTH,(PUCHAR)&size,sizeof(size),&count,0)<0)goto done;
    object=malloc(size);if(!object||BCryptCreateHash(alg,&hash,object,size,NULL,0,0)<0)goto done;
    file=CreateFileW(path,GENERIC_READ,FILE_SHARE_READ,NULL,OPEN_EXISTING,0,NULL);
    if(file==INVALID_HANDLE_VALUE)goto done;
    for(;;){if(!ReadFile(file,bytes,sizeof(bytes),&count,NULL))goto done;if(!count)break;if(BCryptHashData(hash,bytes,count,0)<0)goto done;}
    if(BCryptFinishHash(hash,sum,sizeof(sum),0)<0)goto done;
    for(int i=0;i<32;i++)sprintf(result+2*i,"%02x",sum[i]);result[64]=0;ok=1;
    done: if(file!=INVALID_HANDLE_VALUE)CloseHandle(file);if(hash)BCryptDestroyHash(hash);if(alg)BCryptCloseAlgorithmProvider(alg,0);free(object);return ok;
}
static unsigned long long created(void) {
    FILETIME a,b,c,d;
    if (!GetProcessTimes(GetCurrentProcess(), &a,&b,&c,&d)) return 0;
    return ((unsigned long long)a.dwHighDateTime << 32) | a.dwLowDateTime;
}
static void snapshot(const char *tag) {
    WCHAR text[8192] = {0}, cl[160] = {0};
    DWORD pid = 0;
    RECT r = {0};
    int n = GetWindowTextW(own_window, text, 8192);
    GetClassNameW(own_window, cl, 160);
    GetWindowThreadProcessId(own_window, &pid);
    GetWindowRect(own_window, &r);
    printf("{\"kind\":\"snapshot\",\"tag\":\"%s\",\"pid\":%lu,\"hwnd\":\"%llu\",\"is_unicode\":%d,\"is_visible\":%d,\"is_foreground\":%d,\"wm_settext_count\":%lu,\"caption_utf16le_hex\":\"",
        tag,pid,(unsigned long long)(uintptr_t)own_window,IsWindowUnicode(own_window)!=0,
        IsWindowVisible(own_window)!=0,GetForegroundWindow()==own_window,writes);
    hex(text,(size_t)n*2);
    printf("\",\"class_utf16le_hex\":\""); hex(cl,wcslen(cl)*2);
    printf("\",\"rect\":[%ld,%ld,%ld,%ld]}\n",r.left,r.top,r.right,r.bottom);
    fflush(stdout);
}
static void owner_close(void) {
    snapshot("before_normal_owner_cleanup");
    /* Only the owner thread restores its own fixture during teardown. */
    DefWindowProcW(own_window, WM_SETTEXT, 0, (LPARAM)baseline);
    snapshot("after_normal_owner_cleanup");
    DestroyWindow(own_window);
}
static LRESULT common(HWND h, UINT msg, WPARAM wp, LPARAM lp, int ansi) {
    if (msg == WM_APP + 73) {
        char *command = (char *)lp;
        if (!strncmp(command,"snapshot ",9)) snapshot(command+9);
        else if (!strncmp(command,"thirdparty ",11)) {
            DefWindowProcW(own_window,WM_SETTEXT,0,(LPARAM)third_title);
            snapshot(command+11);
        } else if (!strcmp(command,"close")) owner_close();
        free(command);
        return 0;
    }
    if (msg == WM_TIMER || msg == WM_CLOSE) { owner_close(); return 0; }
    if (msg == WM_DESTROY) { PostQuitMessage(0); return 0; }
    if (msg == WM_SETTEXT && testing) {
        ++writes;
        printf("{\"kind\":\"wm_settext\",\"count\":%lu,\"ansi\":%d}\n",writes,ansi);
        fflush(stdout);
        if (!strcmp(mode,"reject")) return 0;
        if (writes == 1 && !strcmp(mode,"slow650")) Sleep(650);
        if (writes == 1 && !strcmp(mode,"slow2400")) Sleep(2400);
        if (writes == 2 && !strcmp(mode,"corrupt-restore")) {
            WCHAR invalid[] = {0xd800,0};
            return DefWindowProcW(h,msg,wp,(LPARAM)invalid);
        }
    }
    return ansi ? DefWindowProcA(h,msg,wp,lp) : DefWindowProcW(h,msg,wp,lp);
}
static LRESULT CALLBACK proc_w(HWND h,UINT m,WPARAM w,LPARAM l) {return common(h,m,w,l,0);}
static LRESULT CALLBACK proc_a(HWND h,UINT m,WPARAM w,LPARAM l) {return common(h,m,w,l,1);}
static DWORD WINAPI reader(void *unused) {
    (void)unused;
    char line[256];
    while (fgets(line,sizeof(line),stdin)) {
        line[strcspn(line,"\r\n")] = 0;
        char *copy = malloc(strlen(line)+1);
        if (!copy) break;
        strcpy(copy,line);
        if (!PostMessageW(own_window,WM_APP+73,0,(LPARAM)copy)) {free(copy);break;}
    }
    PostMessageW(own_window,WM_CLOSE,0,0);
    return 0;
}
int wmain(int argc, WCHAR **argv) {
    setvbuf(stdout,NULL,_IONBF,0);
    if (argc != 3 || wcsncmp(argv[2],L"LunaOwnedApiFixture-",20) || wcslen(argv[2])<40 || wcslen(argv[2])>=160) return 2;
    if (!WideCharToMultiByte(CP_UTF8,0,argv[1],-1,mode,sizeof(mode),NULL,NULL)) return 3;
    const char *allowed[]={"ansi","unicode","long","invalid","reject","corrupt-restore","slow650","slow2400"};
    int valid=0;for(size_t i=0;i<sizeof(allowed)/sizeof(allowed[0]);i++)if(!strcmp(mode,allowed[i]))valid=1;
    if(!valid)return 4;
    wcscpy(own_class,argv[2]);
    HINSTANCE instance=GetModuleHandleW(NULL);
    if (!strcmp(mode,"ansi")) {
        char cl[320];WideCharToMultiByte(CP_ACP,0,own_class,-1,cl,sizeof(cl),NULL,NULL);
        WNDCLASSA wc={0};wc.hInstance=instance;wc.lpfnWndProc=proc_a;wc.lpszClassName=cl;
        if(!RegisterClassA(&wc))return 5;
        own_window=CreateWindowExA(WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE,cl,"fixture",WS_OVERLAPPEDWINDOW,-30000,-30000,260,140,NULL,NULL,instance,NULL);
    } else {
        WNDCLASSW wc={0};wc.hInstance=instance;wc.lpfnWndProc=proc_w;wc.lpszClassName=own_class;
        if(!RegisterClassW(&wc))return 6;
        own_window=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE,own_class,normal_title,WS_OVERLAPPEDWINDOW,-30000,-30000,260,140,NULL,NULL,instance,NULL);
    }
    if(!own_window)return 7;
    /* Seed exact Unicode caption within owner, including out-of-ACP ANSI case. */
    DefWindowProcW(own_window,WM_SETTEXT,0,(LPARAM)normal_title);
    if(!strcmp(mode,"long")){WCHAR text[4097];for(int i=0;i<4096;i++)text[i]=0x65e5;text[4096]=0;DefWindowProcW(own_window,WM_SETTEXT,0,(LPARAM)text);}
    if(!strcmp(mode,"invalid")){WCHAR text[]={0xd800,0};DefWindowProcW(own_window,WM_SETTEXT,0,(LPARAM)text);}
    if(!strcmp(mode,"corrupt-restore")){WCHAR text[]={0xfffd,' ','f','i','x','t','u','r','e',0};DefWindowProcW(own_window,WM_SETTEXT,0,(LPARAM)text);}
    GetWindowTextW(own_window,baseline,8192);writes=0;testing=1;
    WCHAR path[32768];DWORD len=32768;
    if(!QueryFullProcessImageNameW(GetCurrentProcess(),0,path,&len))return 8;
    char own_sha256[65];if(!image_sha256(path,own_sha256))return 10;
    printf("{\"kind\":\"ready\",\"pid\":%lu,\"bits\":%d,\"hwnd\":\"%llu\",\"creation\":\"%llu\",\"class_utf16le_hex\":\"",GetCurrentProcessId(),(int)(sizeof(void*)*8),(unsigned long long)(uintptr_t)own_window,created());
    hex(own_class,wcslen(own_class)*2);printf("\",\"path_utf16le_hex\":\"");hex(path,wcslen(path)*2);printf("\",\"image_sha256\":\"%s\"}\n",own_sha256);
    snapshot("baseline");
    SetTimer(own_window,1,30000,NULL);
    HANDLE thread=CreateThread(NULL,0,reader,NULL,0,NULL);if(!thread){owner_close();return 9;}CloseHandle(thread);
    MSG message;
    while(GetMessageW(&message,NULL,0,0)>0){TranslateMessage(&message);DispatchMessageW(&message);}
    printf("{\"kind\":\"normal_exit\"}\n");return 0;
}
