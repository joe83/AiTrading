import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard,
  CandlestickChart,
  Briefcase,
  Bot,
  MessageSquareText,
  ScrollText,
  FlaskConical,
  Flame,
  Radar,
  Globe,
  Key,
  Settings,
  LogOut,
  UserCheck,
} from 'lucide-react';
import { useAuthStore } from '../stores/authStore';
import './Sidebar.css';

const NAV_ITEMS = [
  { to: '/',           icon: LayoutDashboard, label: 'Dashboard' },
  { to: '/charts',     icon: CandlestickChart,label: 'Charts' },
  { to: '/positions',  icon: Briefcase,       label: 'Positions' },
  { to: '/signals',    icon: Bot,             label: 'AI Signals' },
  { to: '/agent',      icon: MessageSquareText, label: 'Agent' },
  { to: '/watcher',    icon: Radar,           label: 'Watcher' },
  { to: '/meme-radar', icon: Flame,           label: 'Meme Radar' },
  { to: '/trades',     icon: ScrollText,      label: 'Trade History' },
  { to: '/backtest',   icon: FlaskConical,    label: 'Backtesting' },
  { to: '/exchanges',  icon: Globe,           label: 'Exchanges' },
  { to: '/api-keys',   icon: Key,             label: 'API Keys' },
  { to: '/settings',   icon: Settings,        label: 'Settings' },
];

export function Sidebar() {
  const { username, logout } = useAuthStore();

  return (
    <aside className="sidebar">
      <div className="sidebar__logo">
        <div className="sidebar__logo-icon">
          <Bot size={22} />
        </div>
        <div className="sidebar__logo-text">
          <span className="sidebar__logo-name">AI Trading</span>
          <span className="sidebar__logo-sub">Platform</span>
        </div>
      </div>

      <nav className="sidebar__nav">
        {NAV_ITEMS.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === '/'}
            className={({ isActive }) =>
              `sidebar__link ${isActive ? 'sidebar__link--active' : ''}`
            }
          >
            <item.icon size={19} />
            <span>{item.label}</span>
          </NavLink>
        ))}
      </nav>

      <div className="sidebar__footer">
        <div className="sidebar__user">
          <div className="sidebar__user-info">
            <UserCheck size={16} className="sidebar__user-icon" />
            <span className="sidebar__user-name">{username || 'admin'}</span>
          </div>
          <button
            className="sidebar__logout-btn"
            onClick={logout}
            title="Log out"
            aria-label="Log out"
          >
            <LogOut size={16} />
          </button>
        </div>
        <div className="sidebar__version-row">
          <span className="sidebar__version">v1.0.0</span>
          <span className="sidebar__status-dot" title="Authenticated Session" />
        </div>
      </div>
    </aside>
  );
}
