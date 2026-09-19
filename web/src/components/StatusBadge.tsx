import './StatusBadge.css';

interface StatusBadgeProps {
  status: 'connected' | 'disconnected' | 'auto' | 'manual' | 'paused' | 'active' | 'pending' | 'error';
  label?: string;
  pulse?: boolean;
}

const STATUS_CONFIG: Record<string, { color: string; bg: string; text: string }> = {
  connected:    { color: 'var(--color-profit)',  bg: 'var(--color-profit-dim)',  text: 'Connected' },
  disconnected: { color: 'var(--color-loss)',    bg: 'var(--color-loss-dim)',    text: 'Disconnected' },
  auto:         { color: 'var(--accent-primary)',bg: 'var(--accent-primary-dim)',text: 'Auto' },
  manual:       { color: 'var(--color-warning)', bg: 'var(--color-warning-dim)', text: 'Manual' },
  paused:       { color: 'var(--color-loss)',    bg: 'var(--color-loss-dim)',    text: 'Paused' },
  active:       { color: 'var(--color-profit)',  bg: 'var(--color-profit-dim)',  text: 'Active' },
  pending:      { color: 'var(--color-warning)', bg: 'var(--color-warning-dim)', text: 'Pending' },
  error:        { color: 'var(--color-loss)',    bg: 'var(--color-loss-dim)',    text: 'Error' },
};

export function StatusBadge({ status, label, pulse = false }: StatusBadgeProps) {
  const config = STATUS_CONFIG[status] || STATUS_CONFIG.disconnected;

  return (
    <span
      className={`status-badge ${pulse ? 'status-badge--pulse' : ''}`}
      style={{ color: config.color, backgroundColor: config.bg }}
    >
      <span className="status-badge__dot" style={{ backgroundColor: config.color }} />
      {label || config.text}
    </span>
  );
}
