import { useState, useEffect } from 'react';
import {
  Key,
  Bot,
  Globe,
  TrendingUp,
  DollarSign,
  Eye,
  EyeOff,
  CheckCircle2,
  AlertCircle,
  Loader2,
  Save,
  RefreshCw,
  ExternalLink,
  ShieldCheck,
  Sparkles,
} from 'lucide-react';
import { api, apiClient, type ApiKeysStatus, type GrokLogin, type UpdateApiKeysRequest } from '../api/client';
import './ApiKeysPage.css';

export function ApiKeysPage() {
  const [status, setStatus] = useState<ApiKeysStatus | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [saving, setSaving] = useState<boolean>(false);
  const [saveSuccess, setSaveSuccess] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  // Form states
  const [grokApiKey, setGrokApiKey] = useState('');
  const [grokMode, setGrokMode] = useState<'api' | 'proxy'>('api');
  const [grokAccount, setGrokAccount] = useState<string | null>(null);
  const [grokLogin, setGrokLogin] = useState<GrokLogin | null>(null);
  const [switchingAccount, setSwitchingAccount] = useState(false);
  const [grokBaseUrl, setGrokBaseUrl] = useState('');
  const [grokModelPrimary, setGrokModelPrimary] = useState('');
  const [grokModelFast, setGrokModelFast] = useState('');

  const [mexcApiKey, setMexcApiKey] = useState('');
  const [mexcSecretKey, setMexcSecretKey] = useState('');

  const [binanceApiKey, setBinanceApiKey] = useState('');
  const [binanceSecretKey, setBinanceSecretKey] = useState('');
  const [binanceBaseUrl, setBinanceBaseUrl] = useState('');

  const [bybitApiKey, setBybitApiKey] = useState('');
  const [bybitSecretKey, setBybitSecretKey] = useState('');
  const [bybitBaseUrl, setBybitBaseUrl] = useState('');

  const [alpacaApiKey, setAlpacaApiKey] = useState('');
  const [alpacaSecretKey, setAlpacaSecretKey] = useState('');
  const [alpacaBaseUrl, setAlpacaBaseUrl] = useState('');

  const [icApiKey, setIcApiKey] = useState('');
  const [icAccountId, setIcAccountId] = useState('');
  const [icClientId, setIcClientId] = useState('');
  const [icClientSecret, setIcClientSecret] = useState('');

  // Visibility toggles
  const [showGrokKey, setShowGrokKey] = useState(false);
  const [showMexcSecret, setShowMexcSecret] = useState(false);
  const [showBinanceSecret, setShowBinanceSecret] = useState(false);
  const [showBybitSecret, setShowBybitSecret] = useState(false);
  const [showAlpacaSecret, setShowAlpacaSecret] = useState(false);
  const [showIcSecret, setShowIcSecret] = useState(false);

  // Test connection states
  const [testingService, setTestingService] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, { success: boolean; message: string }>>({});

  const loadStatus = async () => {
    setLoading(true);
    try {
      const data = await apiClient.getApiKeys();
      setStatus(data);
      if (data.grok) {
        setGrokMode(data.grok.mode === 'proxy' ? 'proxy' : 'api');
        setGrokAccount(data.grok.proxy_account ?? null);
        setGrokBaseUrl(
          data.grok.mode === 'proxy' ? 'https://api.x.ai/v1' : data.grok.base_url || 'https://api.x.ai/v1',
        );
        setGrokModelPrimary(data.grok.model_primary || 'grok-4.6');
        setGrokModelFast(data.grok.model_fast || 'grok-4.5');
      }
      if (data.binance) {
        setBinanceBaseUrl(data.binance.base_url || 'https://api.binance.com');
      }
      if (data.bybit) {
        setBybitBaseUrl(data.bybit.base_url || 'https://api.bybit.com');
      }
      if (data.alpaca) {
        setAlpacaBaseUrl(data.alpaca.base_url || 'https://paper-api.alpaca.markets');
      }
      if (data.ic_markets) {
        setIcAccountId(data.ic_markets.account_id || '');
        setIcClientId(data.ic_markets.client_id || '');
      }
    } catch (err: unknown) {
      const e = err as Error;
      setSaveError(`Failed to load API keys: ${e.message || e}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadStatus();
  }, []);

  useEffect(() => {
    if (grokLogin?.status !== 'waiting') return;
    const timer = window.setInterval(async () => {
      try {
        const next = await api.getGrokLogin();
        setGrokLogin(next);
        if (next.status === 'approved') {
          setGrokAccount(next.account ?? null);
          await loadStatus();
        }
      } catch {
        // The next poll retries.
      }
    }, 3000);
    return () => window.clearInterval(timer);
  }, [grokLogin?.status]);

  const changeGrokAccount = async () => {
    setSwitchingAccount(true);
    setSaveError(null);
    try {
      setGrokLogin(await api.startGrokLogin());
    } catch (err: unknown) {
      const e = err as Error;
      setSaveError(e.message || 'Could not start SuperGrok sign-in');
    } finally {
      setSwitchingAccount(false);
    }
  };

  const handleTest = async (service: 'grok' | 'mexc' | 'alpaca' | 'ic_markets' | 'binance' | 'bybit') => {
    setTestingService(service);
    setTestResults((prev) => {
      const copy = { ...prev };
      delete copy[service];
      return copy;
    });

    let key: string | undefined;
    let secret: string | undefined;

    if (service === 'grok') {
      key = grokApiKey || undefined;
    } else if (service === 'mexc') {
      key = mexcApiKey || undefined;
      secret = mexcSecretKey || undefined;
    } else if (service === 'binance') {
      key = binanceApiKey || undefined;
      secret = binanceSecretKey || undefined;
    } else if (service === 'bybit') {
      key = bybitApiKey || undefined;
      secret = bybitSecretKey || undefined;
    } else if (service === 'alpaca') {
      key = alpacaApiKey || undefined;
      secret = alpacaSecretKey || undefined;
    } else if (service === 'ic_markets') {
      key = icApiKey || undefined;
      secret = icClientSecret || undefined;
    }

    try {
      const res = await apiClient.testApiKey({ service, key, secret });
      setTestResults((prev) => ({ ...prev, [service]: res }));
    } catch (err: unknown) {
      const e = err as Error;
      setTestResults((prev) => ({
        ...prev,
        [service]: { success: false, message: e.message || 'Connection test failed' },
      }));
    } finally {
      setTestingService(null);
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setSaveSuccess(null);
    setSaveError(null);

    const payload: UpdateApiKeysRequest = {};

    payload.grok_mode = grokMode;
    if (grokApiKey.trim()) payload.grok_api_key = grokApiKey.trim();
    if (grokMode === 'api' && grokBaseUrl.trim()) payload.grok_base_url = grokBaseUrl.trim();
    if (grokModelPrimary.trim()) payload.grok_model_primary = grokModelPrimary.trim();
    if (grokModelFast.trim()) payload.grok_model_fast = grokModelFast.trim();

    if (mexcApiKey.trim()) payload.mexc_api_key = mexcApiKey.trim();
    if (mexcSecretKey.trim()) payload.mexc_secret_key = mexcSecretKey.trim();

    if (binanceApiKey.trim()) payload.binance_api_key = binanceApiKey.trim();
    if (binanceSecretKey.trim()) payload.binance_secret_key = binanceSecretKey.trim();
    if (binanceBaseUrl.trim()) payload.binance_base_url = binanceBaseUrl.trim();

    if (bybitApiKey.trim()) payload.bybit_api_key = bybitApiKey.trim();
    if (bybitSecretKey.trim()) payload.bybit_secret_key = bybitSecretKey.trim();
    if (bybitBaseUrl.trim()) payload.bybit_base_url = bybitBaseUrl.trim();

    if (alpacaApiKey.trim()) payload.alpaca_api_key = alpacaApiKey.trim();
    if (alpacaSecretKey.trim()) payload.alpaca_secret_key = alpacaSecretKey.trim();
    if (alpacaBaseUrl.trim()) payload.alpaca_base_url = alpacaBaseUrl.trim();

    if (icApiKey.trim()) payload.ic_markets_api_key = icApiKey.trim();
    if (icAccountId.trim()) payload.ic_markets_account_id = icAccountId.trim();
    if (icClientId.trim()) payload.ic_markets_client_id = icClientId.trim();
    if (icClientSecret.trim()) payload.ic_markets_client_secret = icClientSecret.trim();

    try {
      const res = await apiClient.updateApiKeys(payload);
      setSaveSuccess(res.message || 'API keys saved and applied successfully!');
      // Clear sensitive inputs
      setGrokApiKey('');
      setMexcApiKey('');
      setMexcSecretKey('');
      setBinanceApiKey('');
      setBinanceSecretKey('');
      setBybitApiKey('');
      setBybitSecretKey('');
      setAlpacaApiKey('');
      setAlpacaSecretKey('');
      setIcApiKey('');
      setIcClientSecret('');
      // Reload status to reflect new masks
      await loadStatus();
    } catch (err: unknown) {
      const e = err as Error;
      setSaveError(e.message || 'Failed to save API keys');
    } finally {
      setSaving(false);
    }
  };

  if (loading && !status) {
    return (
      <div className="page api-keys-page animate-fade-in">
        <div className="api-keys-page__loading">
          <Loader2 className="spinner" size={32} />
          <span>Loading API keys & integration settings...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="page api-keys-page animate-fade-in">
      <div className="page__header">
        <div className="api-keys-page__header-content">
          <div>
            <h1 className="page__title">API Keys & Integrations</h1>
            <p className="page__subtitle">
              Manage credentials, endpoints, and AI models for Grok AI and broker exchange accounts.
            </p>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={loadStatus} disabled={loading}>
            <RefreshCw size={14} className={loading ? 'spinner' : ''} /> Refresh Status
          </button>
        </div>
      </div>

      {/* Notifications */}
      {saveSuccess && (
        <div className="alert alert-success animate-slide-down">
          <CheckCircle2 size={18} />
          <span>{saveSuccess}</span>
        </div>
      )}
      {saveError && (
        <div className="alert alert-error animate-slide-down">
          <AlertCircle size={18} />
          <span>{saveError}</span>
        </div>
      )}

      <form onSubmit={handleSave} className="api-keys-page__form">
        {/* ================================================================= */}
        {/* 1. Grok AI (xAI) Integration */}
        {/* ================================================================= */}
        <div className="card api-keys-card">
          <div className="api-keys-card__header">
            <div className="api-keys-card__title-box">
              <div className="api-keys-card__icon api-keys-card__icon--grok">
                <Sparkles size={20} />
              </div>
              <div>
                <h2 className="api-keys-card__title">xAI Grok AI Engine</h2>
                <span className="api-keys-card__desc">
                  Core reasoning engine for technical, sentiment, and trade signal generation.
                </span>
              </div>
            </div>
            <div className="api-keys-card__status-badge">
              {status?.grok.mode === 'proxy' ? (
                <span className="badge badge-success">
                  <ShieldCheck size={13} /> SuperGrok{status.grok.proxy_account ? ` (${status.grok.proxy_account})` : ''}
                </span>
              ) : status?.grok.is_set ? (
                <span className="badge badge-success">
                  <ShieldCheck size={13} /> API key ({status.grok.masked_key})
                </span>
              ) : (
                <span className="badge badge-warning">
                  <AlertCircle size={13} /> Not Configured
                </span>
              )}
            </div>
          </div>

          <div className="api-keys-card__body">
            <div className="form-group">
              <label className="form-label">Connection</label>
              <div className="grok-mode">
                <button
                  type="button"
                  className={`btn btn-sm ${grokMode === 'proxy' ? 'btn-primary' : 'btn-secondary'}`}
                  onClick={() => setGrokMode('proxy')}
                >
                  SuperGrok account
                </button>
                <button
                  type="button"
                  className={`btn btn-sm ${grokMode === 'api' ? 'btn-primary' : 'btn-secondary'}`}
                  onClick={() => setGrokMode('api')}
                >
                  xAI API key
                </button>
              </div>
              <span className="form-hint">Save to switch the watcher and the rest of the app.</span>
            </div>

            {grokMode === 'proxy' ? (
              <div className="form-group">
                <label className="form-label">SuperGrok account</label>
                <p className="grok-account">{grokAccount || 'Not signed in'}</p>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => void changeGrokAccount()}
                  disabled={switchingAccount || grokLogin?.status === 'waiting'}
                >
                  {switchingAccount ? 'Starting sign-in...' : 'Change account'}
                </button>
                {grokLogin?.status === 'waiting' && (
                  <div className="grok-login">
                    <p>Open this link and approve the SuperGrok account you want to use.</p>
                    {grokLogin.verification_url ? (
                      <a href={grokLogin.verification_url} target="_blank" rel="noreferrer">
                        {grokLogin.verification_url}
                      </a>
                    ) : (
                      <p className="form-hint">Waiting for the sign-in code...</p>
                    )}
                    {grokLogin.user_code && <p className="grok-login__code">Code: {grokLogin.user_code}</p>}
                  </div>
                )}
                {grokLogin?.status === 'approved' && (
                  <p className="api-keys-test-result api-keys-test-result--success">
                    <CheckCircle2 size={16} /> Signed in{grokLogin.account ? ` as ${grokLogin.account}` : ''}.
                  </p>
                )}
              </div>
            ) : (
            <div className="form-group">
              <label className="form-label">
                Grok API Key
                <span className="form-hint">
                  Obtain from{' '}
                  <a href="https://console.x.ai/" target="_blank" rel="noreferrer" className="external-link">
                    console.x.ai <ExternalLink size={11} />
                  </a>
                </span>
              </label>
              <div className="input-group">
                <input
                  type={showGrokKey ? 'text' : 'password'}
                  className="input"
                  placeholder={status?.grok.is_set ? '•••••••••••••••• (leave blank to keep current)' : 'xai-...'}
                  value={grokApiKey}
                  onChange={(e) => setGrokApiKey(e.target.value)}
                />
                <button
                  type="button"
                  className="input-addon-btn"
                  onClick={() => setShowGrokKey(!showGrokKey)}
                  title={showGrokKey ? 'Hide key' : 'Show key'}
                >
                  {showGrokKey ? <EyeOff size={16} /> : <Eye size={16} />}
                </button>
              </div>
            </div>
            )}

            <div className="api-keys-grid-3">
              {grokMode === 'api' && (
              <div className="form-group">
                <label className="form-label">Base URL</label>
                <input
                  type="text"
                  className="input"
                  placeholder="https://api.x.ai/v1"
                  value={grokBaseUrl}
                  onChange={(e) => setGrokBaseUrl(e.target.value)}
                />
              </div>
              )}

              <div className="form-group">
                <label className="form-label">Primary Reasoning Model</label>
                <input
                  type="text"
                  className="input"
                  placeholder="grok-4.7 or grok-4.6"
                  value={grokModelPrimary}
                  onChange={(e) => setGrokModelPrimary(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label className="form-label">Fast Model (Filters & Backtests)</label>
                <input
                  type="text"
                  className="input"
                  placeholder="grok-4.5"
                  value={grokModelFast}
                  onChange={(e) => setGrokModelFast(e.target.value)}
                />
              </div>
            </div>

            {/* Test result message */}
            {testResults['grok'] && (
              <div
                className={`api-keys-test-result ${
                  testResults['grok'].success ? 'api-keys-test-result--success' : 'api-keys-test-result--error'
                }`}
              >
                {testResults['grok'].success ? <CheckCircle2 size={16} /> : <AlertCircle size={16} />}
                <span>{testResults['grok'].message}</span>
              </div>
            )}

            <div className="api-keys-card__footer">
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => handleTest('grok')}
                disabled={testingService === 'grok'}
              >
                {testingService === 'grok' ? (
                  <>
                    <Loader2 size={14} className="spinner" /> Testing Grok API...
                  </>
                ) : (
                  <>
                    <Bot size={14} /> Test Grok Connection
                  </>
                )}
              </button>
            </div>
          </div>
        </div>

        {/* ================================================================= */}
        {/* 2. MEXC Crypto Exchange */}
        {/* ================================================================= */}
        <div className="card api-keys-card">
          <div className="api-keys-card__header">
            <div className="api-keys-card__title-box">
              <div className="api-keys-card__icon api-keys-card__icon--crypto">
                <span>₿</span>
              </div>
              <div>
                <h2 className="api-keys-card__title">MEXC Global (Crypto)</h2>
                <span className="api-keys-card__desc">
                  Spot & Futures trading for BTC, ETH, SOL, and 1800+ cryptocurrencies.
                </span>
              </div>
            </div>
            <div className="api-keys-card__status-badge">
              {status?.mexc.is_set ? (
                <span className="badge badge-success">
                  <ShieldCheck size={13} /> Active ({status.mexc.masked_api_key})
                </span>
              ) : (
                <span className="badge badge-warning">
                  <AlertCircle size={13} /> Not Configured
                </span>
              )}
            </div>
          </div>

          <div className="api-keys-card__body">
            <div className="api-keys-grid-2">
              <div className="form-group">
                <label className="form-label">MEXC Access API Key</label>
                <input
                  type="text"
                  className="input"
                  placeholder={status?.mexc.is_set ? status.mexc.masked_api_key : 'mx0...'}
                  value={mexcApiKey}
                  onChange={(e) => setMexcApiKey(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label className="form-label">MEXC Secret Key</label>
                <div className="input-group">
                  <input
                    type={showMexcSecret ? 'text' : 'password'}
                    className="input"
                    placeholder={status?.mexc.is_secret_set ? '••••••••••••••••' : 'Enter secret key'}
                    value={mexcSecretKey}
                    onChange={(e) => setMexcSecretKey(e.target.value)}
                  />
                  <button
                    type="button"
                    className="input-addon-btn"
                    onClick={() => setShowMexcSecret(!showMexcSecret)}
                  >
                    {showMexcSecret ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </div>
              </div>
            </div>

            {testResults['mexc'] && (
              <div
                className={`api-keys-test-result ${
                  testResults['mexc'].success ? 'api-keys-test-result--success' : 'api-keys-test-result--error'
                }`}
              >
                {testResults['mexc'].success ? <CheckCircle2 size={16} /> : <AlertCircle size={16} />}
                <span>{testResults['mexc'].message}</span>
              </div>
            )}

            <div className="api-keys-card__footer">
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => handleTest('mexc')}
                disabled={testingService === 'mexc'}
              >
                {testingService === 'mexc' ? (
                  <>
                    <Loader2 size={14} className="spinner" /> Testing MEXC...
                  </>
                ) : (
                  <>
                    <Globe size={14} /> Test MEXC Endpoint
                  </>
                )}
              </button>
            </div>
          </div>
        </div>

        {/* ================================================================= */}
        {/* 3. Binance Crypto Exchange */}
        {/* ================================================================= */}
        <div className="card api-keys-card">
          <div className="api-keys-card__header">
            <div className="api-keys-card__title-box">
              <div className="api-keys-card__icon api-keys-card__icon--binance">
                <span>🔶</span>
              </div>
              <div>
                <h2 className="api-keys-card__title">Binance (Crypto Spot & Futures)</h2>
                <span className="api-keys-card__desc">
                  High-liquidity crypto exchange supporting 1400+ spot and futures markets.
                </span>
              </div>
            </div>
            <div className="api-keys-card__status-badge">
              {status?.binance.is_set ? (
                <span className="badge badge-success">
                  <ShieldCheck size={13} /> Active ({status.binance.masked_api_key})
                </span>
              ) : (
                <span className="badge badge-warning">
                  <AlertCircle size={13} /> Not Configured
                </span>
              )}
            </div>
          </div>

          <div className="api-keys-card__body">
            <div className="api-keys-grid-3">
              <div className="form-group">
                <label className="form-label">Binance API Key</label>
                <input
                  type="text"
                  className="input"
                  placeholder={status?.binance.is_set ? status.binance.masked_api_key : 'Enter Binance API Key'}
                  value={binanceApiKey}
                  onChange={(e) => setBinanceApiKey(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label className="form-label">Binance Secret Key</label>
                <div className="input-group">
                  <input
                    type={showBinanceSecret ? 'text' : 'password'}
                    className="input"
                    placeholder={status?.binance.is_secret_set ? '••••••••••••••••' : 'Enter secret key'}
                    value={binanceSecretKey}
                    onChange={(e) => setBinanceSecretKey(e.target.value)}
                  />
                  <button
                    type="button"
                    className="input-addon-btn"
                    onClick={() => setShowBinanceSecret(!showBinanceSecret)}
                  >
                    {showBinanceSecret ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </div>
              </div>

              <div className="form-group">
                <label className="form-label">Base REST URL</label>
                <input
                  type="text"
                  className="input"
                  placeholder="https://api.binance.com"
                  value={binanceBaseUrl}
                  onChange={(e) => setBinanceBaseUrl(e.target.value)}
                />
              </div>
            </div>

            {testResults['binance'] && (
              <div
                className={`api-keys-test-result ${
                  testResults['binance'].success ? 'api-keys-test-result--success' : 'api-keys-test-result--error'
                }`}
              >
                {testResults['binance'].success ? <CheckCircle2 size={16} /> : <AlertCircle size={16} />}
                <span>{testResults['binance'].message}</span>
              </div>
            )}

            <div className="api-keys-card__footer">
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => handleTest('binance')}
                disabled={testingService === 'binance'}
              >
                {testingService === 'binance' ? (
                  <>
                    <Loader2 size={14} className="spinner" /> Testing Binance...
                  </>
                ) : (
                  <>
                    <Globe size={14} /> Test Binance Connection
                  </>
                )}
              </button>
            </div>
          </div>
        </div>

        {/* ================================================================= */}
        {/* 4. Bybit Crypto Exchange (V5 API) */}
        {/* ================================================================= */}
        <div className="card api-keys-card">
          <div className="api-keys-card__header">
            <div className="api-keys-card__title-box">
              <div className="api-keys-card__icon api-keys-card__icon--bybit">
                <span>🟡</span>
              </div>
              <div>
                <h2 className="api-keys-card__title">Bybit (Unified Trading V5 API)</h2>
                <span className="api-keys-card__desc">
                  Unified account architecture for Spot and Derivatives trading on Bybit.
                </span>
              </div>
            </div>
            <div className="api-keys-card__status-badge">
              {status?.bybit.is_set ? (
                <span className="badge badge-success">
                  <ShieldCheck size={13} /> Active ({status.bybit.masked_api_key})
                </span>
              ) : (
                <span className="badge badge-warning">
                  <AlertCircle size={13} /> Not Configured
                </span>
              )}
            </div>
          </div>

          <div className="api-keys-card__body">
            <div className="api-keys-grid-3">
              <div className="form-group">
                <label className="form-label">Bybit API Key</label>
                <input
                  type="text"
                  className="input"
                  placeholder={status?.bybit.is_set ? status.bybit.masked_api_key : 'Enter Bybit API Key'}
                  value={bybitApiKey}
                  onChange={(e) => setBybitApiKey(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label className="form-label">Bybit Secret Key</label>
                <div className="input-group">
                  <input
                    type={showBybitSecret ? 'text' : 'password'}
                    className="input"
                    placeholder={status?.bybit.is_secret_set ? '••••••••••••••••' : 'Enter secret key'}
                    value={bybitSecretKey}
                    onChange={(e) => setBybitSecretKey(e.target.value)}
                  />
                  <button
                    type="button"
                    className="input-addon-btn"
                    onClick={() => setShowBybitSecret(!showBybitSecret)}
                  >
                    {showBybitSecret ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </div>
              </div>

              <div className="form-group">
                <label className="form-label">Base REST URL</label>
                <input
                  type="text"
                  className="input"
                  placeholder="https://api.bybit.com"
                  value={bybitBaseUrl}
                  onChange={(e) => setBybitBaseUrl(e.target.value)}
                />
              </div>
            </div>

            {testResults['bybit'] && (
              <div
                className={`api-keys-test-result ${
                  testResults['bybit'].success ? 'api-keys-test-result--success' : 'api-keys-test-result--error'
                }`}
              >
                {testResults['bybit'].success ? <CheckCircle2 size={16} /> : <AlertCircle size={16} />}
                <span>{testResults['bybit'].message}</span>
              </div>
            )}

            <div className="api-keys-card__footer">
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => handleTest('bybit')}
                disabled={testingService === 'bybit'}
              >
                {testingService === 'bybit' ? (
                  <>
                    <Loader2 size={14} className="spinner" /> Testing Bybit...
                  </>
                ) : (
                  <>
                    <Globe size={14} /> Test Bybit Connection
                  </>
                )}
              </button>
            </div>
          </div>
        </div>

        {/* ================================================================= */}
        {/* 5. Alpaca Equities & Stocks */}
        {/* ================================================================= */}
        <div className="card api-keys-card">
          <div className="api-keys-card__header">
            <div className="api-keys-card__title-box">
              <div className="api-keys-card__icon api-keys-card__icon--stocks">
                <TrendingUp size={20} />
              </div>
              <div>
                <h2 className="api-keys-card__title">Alpaca Markets (US Equities)</h2>
                <span className="api-keys-card__desc">
                  Commission-free trading for US Stocks, ETFs, and market data (Paper & Live).
                </span>
              </div>
            </div>
            <div className="api-keys-card__status-badge">
              {status?.alpaca.is_set ? (
                <span className="badge badge-success">
                  <ShieldCheck size={13} /> Active ({status.alpaca.masked_api_key})
                </span>
              ) : (
                <span className="badge badge-warning">
                  <AlertCircle size={13} /> Not Configured
                </span>
              )}
            </div>
          </div>

          <div className="api-keys-card__body">
            <div className="api-keys-grid-3">
              <div className="form-group">
                <label className="form-label">Alpaca API Key ID</label>
                <input
                  type="text"
                  className="input"
                  placeholder={status?.alpaca.is_set ? status.alpaca.masked_api_key : 'PK...'}
                  value={alpacaApiKey}
                  onChange={(e) => setAlpacaApiKey(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label className="form-label">Alpaca Secret Key</label>
                <div className="input-group">
                  <input
                    type={showAlpacaSecret ? 'text' : 'password'}
                    className="input"
                    placeholder={status?.alpaca.is_secret_set ? '••••••••••••••••' : 'Enter secret key'}
                    value={alpacaSecretKey}
                    onChange={(e) => setAlpacaSecretKey(e.target.value)}
                  />
                  <button
                    type="button"
                    className="input-addon-btn"
                    onClick={() => setShowAlpacaSecret(!showAlpacaSecret)}
                  >
                    {showAlpacaSecret ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </div>
              </div>

              <div className="form-group">
                <label className="form-label">Endpoint URL</label>
                <input
                  type="text"
                  className="input"
                  placeholder="https://paper-api.alpaca.markets"
                  value={alpacaBaseUrl}
                  onChange={(e) => setAlpacaBaseUrl(e.target.value)}
                />
              </div>
            </div>

            {testResults['alpaca'] && (
              <div
                className={`api-keys-test-result ${
                  testResults['alpaca'].success ? 'api-keys-test-result--success' : 'api-keys-test-result--error'
                }`}
              >
                {testResults['alpaca'].success ? <CheckCircle2 size={16} /> : <AlertCircle size={16} />}
                <span>{testResults['alpaca'].message}</span>
              </div>
            )}

            <div className="api-keys-card__footer">
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => handleTest('alpaca')}
                disabled={testingService === 'alpaca'}
              >
                {testingService === 'alpaca' ? (
                  <>
                    <Loader2 size={14} className="spinner" /> Verifying Alpaca...
                  </>
                ) : (
                  <>
                    <TrendingUp size={14} /> Test Alpaca Account
                  </>
                )}
              </button>
            </div>
          </div>
        </div>

        {/* ================================================================= */}
        {/* 4. IC Markets Forex */}
        {/* ================================================================= */}
        <div className="card api-keys-card">
          <div className="api-keys-card__header">
            <div className="api-keys-card__title-box">
              <div className="api-keys-card__icon api-keys-card__icon--forex">
                <DollarSign size={20} />
              </div>
              <div>
                <h2 className="api-keys-card__title">IC Markets (Forex / cTrader)</h2>
                <span className="api-keys-card__desc">
                  OpenAPI integration for Forex pairs (EUR/USD, GBP/USD, commodities).
                </span>
              </div>
            </div>
            <div className="api-keys-card__status-badge">
              {status?.ic_markets.is_set ? (
                <span className="badge badge-success">
                  <ShieldCheck size={13} /> Configured
                </span>
              ) : (
                <span className="badge badge-warning">
                  <AlertCircle size={13} /> Not Configured
                </span>
              )}
            </div>
          </div>

          <div className="api-keys-card__body">
            <div className="api-keys-grid-2">
              <div className="form-group">
                <label className="form-label">IC Markets API Key</label>
                <input
                  type="text"
                  className="input"
                  placeholder={status?.ic_markets.is_set ? status.ic_markets.masked_api_key : 'Enter API key'}
                  value={icApiKey}
                  onChange={(e) => setIcApiKey(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label className="form-label">Account ID</label>
                <input
                  type="text"
                  className="input"
                  placeholder="e.g. 12345678"
                  value={icAccountId}
                  onChange={(e) => setIcAccountId(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label className="form-label">Client ID</label>
                <input
                  type="text"
                  className="input"
                  placeholder="OpenAPI Client ID"
                  value={icClientId}
                  onChange={(e) => setIcClientId(e.target.value)}
                />
              </div>

              <div className="form-group">
                <label className="form-label">Client Secret</label>
                <div className="input-group">
                  <input
                    type={showIcSecret ? 'text' : 'password'}
                    className="input"
                    placeholder={status?.ic_markets.is_client_secret_set ? '••••••••••••••••' : 'Enter client secret'}
                    value={icClientSecret}
                    onChange={(e) => setIcClientSecret(e.target.value)}
                  />
                  <button
                    type="button"
                    className="input-addon-btn"
                    onClick={() => setShowIcSecret(!showIcSecret)}
                  >
                    {showIcSecret ? <EyeOff size={16} /> : <Eye size={16} />}
                  </button>
                </div>
              </div>
            </div>

            {testResults['ic_markets'] && (
              <div
                className={`api-keys-test-result ${
                  testResults['ic_markets'].success ? 'api-keys-test-result--success' : 'api-keys-test-result--error'
                }`}
              >
                {testResults['ic_markets'].success ? <CheckCircle2 size={16} /> : <AlertCircle size={16} />}
                <span>{testResults['ic_markets'].message}</span>
              </div>
            )}
          </div>
        </div>

        {/* ================================================================= */}
        {/* Sticky Submit Bar */}
        {/* ================================================================= */}
        <div className="api-keys-page__actions">
          <div className="api-keys-page__actions-info">
            <Key size={18} />
            <span>
              Saved keys are hot-reloaded into memory immediately and persisted to the server's <code>.env</code> file.
            </span>
          </div>
          <button type="submit" className="btn btn-success btn-lg api-keys-page__submit-btn" disabled={saving}>
            {saving ? (
              <>
                <Loader2 size={18} className="spinner" /> Saving & Applying Keys...
              </>
            ) : (
              <>
                <Save size={18} /> Save & Apply All Keys
              </>
            )}
          </button>
        </div>
      </form>
    </div>
  );
}
