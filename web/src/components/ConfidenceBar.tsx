import './ConfidenceBar.css';

interface ConfidenceBarProps {
  value: number; // 0-1 or 0-100
  label?: string;
  showValue?: boolean;
  size?: 'sm' | 'md';
}

export function ConfidenceBar({ value, label, showValue = true, size = 'md' }: ConfidenceBarProps) {
  const pct = value > 1 ? value : value * 100;
  const getColor = () => {
    if (pct >= 75) return 'var(--color-profit)';
    if (pct >= 50) return 'var(--accent-primary)';
    if (pct >= 30) return 'var(--color-warning)';
    return 'var(--color-loss)';
  };

  return (
    <div className={`confidence confidence--${size}`}>
      {(label || showValue) && (
        <div className="confidence__header">
          {label && <span className="confidence__label">{label}</span>}
          {showValue && <span className="confidence__value" style={{ color: getColor() }}>{pct.toFixed(0)}%</span>}
        </div>
      )}
      <div className="confidence__track">
        <div
          className="confidence__fill"
          style={{ width: `${Math.min(pct, 100)}%`, backgroundColor: getColor() }}
        />
      </div>
    </div>
  );
}
