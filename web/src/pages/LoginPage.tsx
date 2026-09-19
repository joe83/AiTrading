import { useState, type FormEvent } from 'react';
import { Bot, Eye, EyeOff, Lock, User, AlertCircle } from 'lucide-react';
import { useAuthStore } from '../stores/authStore';
import { useNavigate } from 'react-router-dom';
import './LoginPage.css';

export function LoginPage() {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const { login, isLoading, error } = useAuthStore();
  const navigate = useNavigate();

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const success = await login(username, password);
    if (success) {
      navigate('/');
    }
  };

  return (
    <div className="login-page">
      {/* Animated background */}
      <div className="login-page__bg">
        <div className="login-page__grid" />
        <div className="login-page__glow login-page__glow--1" />
        <div className="login-page__glow login-page__glow--2" />
      </div>

      <div className="login-page__card animate-fade-in">
        {/* Logo */}
        <div className="login-page__logo">
          <div className="login-page__logo-icon">
            <Bot size={28} />
          </div>
          <h1 className="login-page__title">AI Trading</h1>
          <p className="login-page__subtitle">Platform</p>
        </div>

        <p className="login-page__desc">Sign in to access your trading dashboard</p>

        {/* Error Message */}
        {error && (
          <div className="login-page__error">
            <AlertCircle size={16} />
            <span>{error}</span>
          </div>
        )}

        {/* Form */}
        <form onSubmit={handleSubmit} className="login-page__form">
          <div className="login-page__field">
            <label className="label" htmlFor="username">Username</label>
            <div className="login-page__input-wrapper">
              <User size={16} className="login-page__input-icon" />
              <input
                id="username"
                type="text"
                className="input login-page__input"
                placeholder="Enter username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                autoComplete="username"
                autoFocus
                required
              />
            </div>
          </div>

          <div className="login-page__field">
            <label className="label" htmlFor="password">Password</label>
            <div className="login-page__input-wrapper">
              <Lock size={16} className="login-page__input-icon" />
              <input
                id="password"
                type={showPassword ? 'text' : 'password'}
                className="input login-page__input"
                placeholder="Enter password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="current-password"
                required
              />
              <button
                type="button"
                className="login-page__toggle-pw"
                onClick={() => setShowPassword(!showPassword)}
                tabIndex={-1}
              >
                {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
          </div>

          <button
            type="submit"
            className="btn btn-primary btn-lg w-full login-page__submit"
            disabled={isLoading || !username || !password}
          >
            {isLoading ? (
              <span className="login-page__spinner" />
            ) : (
              <>
                <Lock size={16} /> Sign In
              </>
            )}
          </button>
        </form>

        <p className="login-page__footer">
          Secured with JWT authentication
        </p>
      </div>
    </div>
  );
}
