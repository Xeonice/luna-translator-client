import { Component } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { EmptyState, Notice } from '../ui/patterns/Patterns';
import { Button } from '../ui/primitives/Controls';

export class ErrorBoundary extends Component<{ children: ReactNode }, { message: string | null }> {
  state: { message: string | null } = { message: null };
  static getDerivedStateFromError(error: Error) { return { message: error.message }; }
  componentDidCatch(_error: Error, _info: ErrorInfo) { /* React contains the rendering failure; business state stays in Rust. */ }
  render() { return this.state.message ? <EmptyState title="这个窗口未能显示" description="应用状态与游戏连接仍由后台保留。可以重新载入这个窗口。"><Notice error>{this.state.message}</Notice><Button onPress={() => location.reload()}>重新载入窗口</Button></EmptyState> : this.props.children; }
}
