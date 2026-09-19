import { X, CheckCircle, AlertTriangle, AlertCircle, Info } from 'lucide-react';
import { useTradingStore } from '../stores/tradingStore';
import './Toast.css';

const ICONS = {
  success: <CheckCircle size={18} />,
  error: <AlertCircle size={18} />,
  warning: <AlertTriangle size={18} />,
  info: <Info size={18} />,
};

export function ToastContainer() {
  const toasts = useTradingStore((s) => s.toasts);
  const removeToast = useTradingStore((s) => s.removeToast);

  if (toasts.length === 0) return null;

  return (
    <div className="toast-container">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast toast--${toast.type} animate-slide-in`}>
          <div className="toast__icon">{ICONS[toast.type]}</div>
          <div className="toast__content">
            <div className="toast__title">{toast.title}</div>
            <div className="toast__message">{toast.message}</div>
          </div>
          <button className="toast__close" onClick={() => removeToast(toast.id)}>
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
