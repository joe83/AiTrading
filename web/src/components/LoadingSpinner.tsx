import './LoadingSpinner.css';

interface LoadingSpinnerProps {
  size?: number;
  text?: string;
}

export function LoadingSpinner({ size = 32, text }: LoadingSpinnerProps) {
  return (
    <div className="loading-spinner">
      <svg width={size} height={size} viewBox="0 0 24 24" className="loading-spinner__svg">
        <circle cx="12" cy="12" r="10" fill="none" stroke="var(--border-secondary)" strokeWidth="2" />
        <circle cx="12" cy="12" r="10" fill="none" stroke="var(--accent-primary)" strokeWidth="2"
          strokeDasharray="31.42" strokeDashoffset="10" strokeLinecap="round" />
      </svg>
      {text && <span className="loading-spinner__text">{text}</span>}
    </div>
  );
}
