import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { useAuthStore } from './stores/authStore';
import { DashboardLayout } from './layouts/DashboardLayout';
import { LoginPage } from './pages/LoginPage';
import { DashboardPage } from './pages/DashboardPage';
import { ChartsPage } from './pages/ChartsPage';
import { PositionsPage } from './pages/PositionsPage';
import { SignalsPage } from './pages/SignalsPage';
import { TradeHistoryPage } from './pages/TradeHistoryPage';
import { BacktestPage } from './pages/BacktestPage';
import { ExchangesPage } from './pages/ExchangesPage';
import { SettingsPage } from './pages/SettingsPage';

/**
 * Protected route wrapper — redirects to /login if not authenticated.
 */
function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
}

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        {/* Public route — login */}
        <Route path="/login" element={<LoginPage />} />

        {/* Protected routes — require auth */}
        <Route
          element={
            <ProtectedRoute>
              <DashboardLayout />
            </ProtectedRoute>
          }
        >
          <Route path="/" element={<DashboardPage />} />
          <Route path="/charts" element={<ChartsPage />} />
          <Route path="/positions" element={<PositionsPage />} />
          <Route path="/signals" element={<SignalsPage />} />
          <Route path="/trades" element={<TradeHistoryPage />} />
          <Route path="/backtest" element={<BacktestPage />} />
          <Route path="/exchanges" element={<ExchangesPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>

        {/* Catch-all → redirect to dashboard (which will redirect to login if not auth'd) */}
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
