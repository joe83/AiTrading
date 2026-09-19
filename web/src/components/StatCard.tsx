import { TrendingUp, TrendingDown, Minus } from 'lucide-react';
import './StatCard.css';

interface StatCardProps {
  label: string;
  value: string | number;
  change?: number;
  prefix?: string;
  suffix?: string;
  icon?: React.ReactNode;
  variant?: 'default' | 'profit' | 'loss';
}

export function StatCard({ label, value, change, prefix, suffix, icon, variant = 'default' }: StatCardProps) {
  const getChangeColor = () => {
    if (change === undefined) return '';
    if (change > 0) return 'text-profit';
    if (change < 0) return 'text-loss';
    return 'text-muted';
  };

  const getChangeIcon = () => {
    if (change === undefined) return null;
    if (change > 0) return <TrendingUp size={14} />;
    if (change < 0) return <TrendingDown size={14} />;
    return <Minus size={14} />;
  };

  return (
    <div className={`stat-card stat-card--${variant}`}>
      <div className="stat-card__header">
        <span className="stat-card__label">{label}</span>
        {icon && <span className="stat-card__icon">{icon}</span>}
      </div>
      <div className="stat-card__value">
        {prefix && <span className="stat-card__prefix">{prefix}</span>}
        <span>{typeof value === 'number' ? value.toLocaleString() : value}</span>
        {suffix && <span className="stat-card__suffix">{suffix}</span>}
      </div>
      {change !== undefined && (
        <div className={`stat-card__change ${getChangeColor()}`}>
          {getChangeIcon()}
          <span>{change > 0 ? '+' : ''}{change.toFixed(2)}%</span>
        </div>
      )}
    </div>
  );
}
