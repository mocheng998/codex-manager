import { createRoot } from "react-dom/client";
import { Component, type ReactNode } from "react";
import { App } from "./App";
import "./styles.css";

class AppErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <main className="appError">
          <section>
            <h1>页面渲染失败</h1>
            <p>{this.state.error.message || "未知错误"}</p>
            <button type="button" onClick={() => window.location.reload()}>
              重新加载
            </button>
          </section>
        </main>
      );
    }
    return this.props.children;
  }
}

const root = document.getElementById("root");

if (root instanceof HTMLElement) {
  createRoot(root).render(
    <AppErrorBoundary>
      <App />
    </AppErrorBoundary>,
  );
}
