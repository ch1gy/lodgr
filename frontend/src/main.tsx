// ─────────────────────────────────────────────────────────────────────────────
// main.tsx — Vite entry point.
//
// We apply the theme synchronously BEFORE React mounts so users on dark
// systems never see a light-mode flash. Order matters here:
//   1. import the global stylesheet (so tokens are available)
//   2. set [data-theme] on <html>
//   3. mount React
// ─────────────────────────────────────────────────────────────────────────────

import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { applyInitialThemeSync } from './theme/ThemeContext';
import './styles/tokens.css';

applyInitialThemeSync();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
