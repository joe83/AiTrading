import './PnlDisplay.css';

interface PnlDisplayProps {
  value: number | string;
  showSign?: boolean;
  size?: 'sm' | 'md' | 'lg';
  suffix?: string;
}

export function PnlDisplay({ value, showSign = true, size = 'md', suffix = '' }: PnlDisplayProps) {
  const numVal = typeof value === 'string' ? parseFloat(value) : value;
  const isProfit = numVal > 0;
  const isLoss = numVal < 0;
  const isNeutral = numVal === 0 || isNaN(numVal);

  const colorClass = isProfit ? 'pnl--profit' : isLoss ? 'pnl--loss' : 'pnl--neutral';
  const sign = showSign && isProfit ? '+' : '';

  return (
    <span className={`pnl pnl--${size} ${colorClass}`}>
      {sign}{isNaN(numVal) ? '0.00' : numVal.toFixed(2)}{suffix}
      {!isNeutral && (
        <span className="pnl__arrow">{isProfit ? '▲' : '▼'}</span>
      )}
    </span>
  );
}
