import { useCallback, useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import {
  Radar,
  Zap,
  Settings,
  RefreshCw,
  Play,
  Pause,
  Copy,
  Check,
  Sparkles,
  Send,
  ShieldCheck,
  AlertCircle,
  HelpCircle,
} from 'lucide-react';
import {
  api,
  type GrokAccount,
  type WatchConfig,
  type WatchIngestResult,
  type WatchSnapshot,
} from '../api/client';
import './WatcherPage.css';

export function WatcherPage() {
  const [watch, setWatch] = useState<WatchSnapshot | null>(null);
  const [account, setAccount] = useState<GrokAccount | null>(null);
  const [_config, setConfig] = useState<WatchConfig | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [copiedWebhook, setCopiedWebhook] = useState(false);

  // Editable config state
  const [provider, setProvider] = useState<'webhook' | 'scraper' | 'grok'>('webhook');
  const [enabled, setEnabled] = useState(true);
  const [intervalSecs, setIntervalSecs] = useState(300);
  const [handlesInput, setHandlesInput] = useState('elonmusk, realDonaldTrump');
  const [minConfidence, setMinConfidence] = useState(0.7);
  const [scraperProvider, setScraperProvider] = useState<'twitterapi_io' | 'rapidapi' | 'custom'>('twitterapi_io');
  const [scraperApiKey, setScraperApiKey] = useState('');
  const [customFeedUrl, setCustomFeedUrl] = useState('');
  const [webhookSecret, setWebhookSecret] = useState('');

  // Interactive Test Ingest state
  const [testHandle, setTestHandle] = useState('elonmusk');
  const [testText, setTestText] = useState('Tesla Robotaxi autonomous fleet approved for launch next week in Austin.');
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<WatchIngestResult | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [nextWatch, nextAccount, nextConfig] = await Promise.all([
        api.getWatch(),
        api.getGrokAccount(),
        api.getWatchConfig().catch(() => null),
      ]);
      setWatch(nextWatch);
      setAccount(nextAccount);
      if (nextConfig) {
        setConfig(nextConfig);
        setEnabled(nextConfig.enabled);
        setProvider((nextConfig.provider as 'webhook' | 'scraper' | 'grok') || 'webhook');
        setIntervalSecs(nextConfig.interval_secs || 300);
        setHandlesInput((nextConfig.handles || []).join(', '));
        setMinConfidence(nextConfig.min_confidence || 0.7);
        setScraperProvider((nextConfig.scraper_provider as 'twitterapi_io' | 'rapidapi' | 'custom') || 'twitterapi_io');
        setScraperApiKey(nextConfig.scraper_api_key || '');
        setCustomFeedUrl(nextConfig.custom_feed_url || '');
        setWebhookSecret(nextConfig.webhook_secret || '');
      }
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not load the watcher');
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 15000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const handleSaveConfig = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    setIsSaving(true);
    setError(null);
    setSuccess(null);
    try {
      const handles = handlesInput
        .split(',')
        .map((h) => h.trim().replace(/^@/, ''))
        .filter(Boolean);

      const res = await api.updateWatchConfig({
        enabled,
        provider,
        interval_secs: Number(intervalSecs),
        handles,
        min_confidence: Number(minConfidence),
        scraper_provider: scraperProvider,
        scraper_api_key: scraperApiKey,
        custom_feed_url: customFeedUrl,
        webhook_secret: webhookSecret,
      });
      setConfig(res.config);
      setSuccess('Watcher configuration updated successfully');
      void refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update watcher settings');
    } finally {
      setIsSaving(false);
    }
  };

  const handleTriggerScan = async () => {
    setIsScanning(true);
    setError(null);
    setSuccess(null);
    try {
      const res = await api.triggerWatchScan();
      setSuccess(`Scan finished: ${res.result}`);
      void refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Manual scan failed');
    } finally {
      setIsScanning(false);
    }
  };

  const handleTestIngest = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!testHandle || !testText) return;
    setIsTesting(true);
    setTestResult(null);
    setError(null);
    try {
      const res = await api.ingestWatchPost({
        handle: testHandle,
        text: testText,
      });
      setTestResult(res);
      void refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Test post ingest failed');
    } finally {
      setIsTesting(false);
    }
  };

  const webhookEndpoint = `${window.location.origin}/api/watch/ingest`;

  const copyWebhookUrl = () => {
    navigator.clipboard.writeText(webhookEndpoint);
    setCopiedWebhook(true);
    setTimeout(() => setCopiedWebhook(false), 2000);
  };

  const status = watch?.status;

  return (
    <div className="page watcher-page animate-fade-in">
      <div className="page__header">
        <div className="watcher-header__title-wrap">
          <h1 className="page__title">X Watcher</h1>
          <span className="badge badge--success">Filter First, AI Second</span>
        </div>
        <p className="page__subtitle">
          Affordable social sentiment monitoring. Free/cheap tweet filtering prevents continuous $0.35 xAI polling fees—invoking LLM reasoning only when brand new posts appear.
        </p>
      </div>

      {error && <div className="alert alert--danger watcher-banner">{error}</div>}
      {success && <div className="alert alert--success watcher-banner">{success}</div>}

      {/* Architecture Cost Comparison Card */}
      <section className="watcher-architecture card">
        <div className="watcher-architecture__header">
          <Zap className="watcher-accent-icon" size={20} />
          <div>
            <h2 className="card__title">How "Filter First, AI Second" Saves 99% Costs</h2>
            <p className="text-muted">
              Continuous Grok live-search costs ~$0.35–$0.47 per check ($500+/mo). By decoupling tweet ingestion from AI reasoning:
            </p>
          </div>
        </div>
        <div className="watcher-architecture__steps">
          <div className="arch-step">
            <span className="arch-step__num">1</span>
            <div className="arch-step__body">
              <strong>Lightweight Ingestion</strong>
              <p>Webhook or low-cost scraper checks if a post exists</p>
              <span className="arch-step__cost">$0.00 / check</span>
            </div>
          </div>
          <div className="arch-step__arrow">→</div>
          <div className="arch-step">
            <span className="arch-step__num">2</span>
            <div className="arch-step__body">
              <strong>TimescaleDB Dedup</strong>
              <p>Post ID & age checked; skips already-seen or old tweets</p>
              <span className="arch-step__cost">$0.00 / check</span>
            </div>
          </div>
          <div className="arch-step__arrow">→</div>
          <div className="arch-step arch-step--highlight">
            <span className="arch-step__num">3</span>
            <div className="arch-step__body">
              <strong>AI Trade Reasoning</strong>
              <p>Evaluates asset impact, tilt & generates signal only on new posts</p>
              <span className="arch-step__cost">~$0.0002 / post</span>
            </div>
          </div>
        </div>
      </section>

      {/* Status & Connection Overview */}
      <div className="watcher-grid">
        <section className="card watcher-status-card">
          <div className="watcher-status-card__top">
            <div className="watcher-status-card__radar">
              <Radar className={status?.running ? 'radar-pulse' : 'text-muted'} size={24} />
              <div>
                <h2 className="card__title">
                  {status?.running ? 'Watcher Active' : 'Watcher Paused'}
                </h2>
                <span className="watcher-mode-badge">
                  Provider: <strong>{status?.provider || provider}</strong>
                </span>
              </div>
            </div>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={handleTriggerScan}
              disabled={isScanning || !status?.running}
            >
              <RefreshCw size={14} className={isScanning ? 'animate-spin' : ''} />
              {isScanning ? 'Scanning...' : 'Scan Now'}
            </button>
          </div>

          <div className="watcher-status-card__meta">
            <p className="watcher-last-result">
              <strong>Last Result:</strong> {status?.last_result || 'No scan yet'}
            </p>
            <p className="text-muted">
              <strong>Monitored Accounts:</strong>{' '}
              {status?.handles?.length
                ? status.handles.map((h) => `@${h}`).join(', ')
                : 'None configured'}
            </p>
            <p className="text-muted">
              <strong>Cycle Interval:</strong> {status?.interval_secs ? `${status.interval_secs}s` : '600s'}
              {status?.last_tick_at && ` · Last tick: ${new Date(status.last_tick_at).toLocaleTimeString()}`}
            </p>
            {status?.last_error && <p className="watcher-page__error">{status.last_error}</p>}
          </div>

          <div className="watcher-ai-account">
            <div>
              <span className="text-muted text-xs">AI Reasoning Engine</span>
              <p className="watcher-ai-account__name">
                {account?.logged_in ? `SuperGrok (${account.account})` : 'xAI API Key (Fast JSON mode)'}
              </p>
            </div>
            <Link to="/api-keys" className="btn btn-secondary btn-xs">
              Manage Keys
            </Link>
          </div>
        </section>

        {/* Interactive Ingest & AI Test Panel */}
        <section className="card watcher-test-card">
          <div className="watcher-test-card__header">
            <Sparkles size={18} className="text-accent" />
            <h2 className="card__title">Test Tweet Ingest & Reasoning</h2>
          </div>
          <p className="text-muted text-sm">
            Simulate an incoming tweet without waiting. Tests the deduplication and AI trading evaluation instantly.
          </p>

          <form onSubmit={handleTestIngest} className="watcher-test-form">
            <div className="form-group">
              <label htmlFor="test-handle">Account Handle</label>
              <div className="input-prefix-wrap">
                <span className="input-prefix">@</span>
                <input
                  id="test-handle"
                  type="text"
                  className="input"
                  value={testHandle}
                  onChange={(e) => setTestHandle(e.target.value)}
                  placeholder="elonmusk"
                  required
                />
              </div>
            </div>

            <div className="form-group">
              <label htmlFor="test-text">Tweet Content</label>
              <textarea
                id="test-text"
                className="input watcher-test-textarea"
                rows={2}
                value={testText}
                onChange={(e) => setTestText(e.target.value)}
                placeholder="Type or paste tweet text..."
                required
              />
            </div>

            <button type="submit" className="btn btn-primary btn-sm" disabled={isTesting}>
              <Send size={14} />
              {isTesting ? 'Evaluating with AI...' : 'Analyze & Ingest Post'}
            </button>
          </form>

          {testResult && (
            <div className={`watcher-test-output ${testResult.queued ? 'queued' : 'not-queued'}`}>
              <div className="watcher-test-output__header">
                <strong>Result:</strong>
                {testResult.already_seen ? (
                  <span className="badge badge--warning">Already Seen ($0 AI tokens)</span>
                ) : testResult.is_stale ? (
                  <span className="badge badge--warning">Stale Post ($0 AI tokens)</span>
                ) : testResult.queued ? (
                  <span className="badge badge--success">Signal Queued!</span>
                ) : (
                  <span className="badge badge--secondary">Not actionable</span>
                )}
              </div>
              <div className="watcher-test-output__grid">
                <div>
                  <span className="text-muted">Symbol:</span>{' '}
                  <strong>{testResult.symbol || 'None'}</strong>
                </div>
                <div>
                  <span className="text-muted">Side:</span>{' '}
                  <strong>{testResult.side ? testResult.side.toUpperCase() : 'None'}</strong>
                </div>
                <div>
                  <span className="text-muted">Confidence:</span>{' '}
                  <strong>
                    {testResult.confidence !== null && testResult.confidence !== undefined
                      ? `${Math.round(testResult.confidence * 100)}%`
                      : 'N/A'}
                  </strong>
                </div>
              </div>
              <p className="watcher-test-output__reason">
                <span className="text-muted">Reason:</span> {testResult.reason}
              </p>
            </div>
          )}
        </section>
      </div>

      {/* Configuration & Provider Settings */}
      <section className="card watcher-config-card">
        <div className="watcher-config-card__header">
          <Settings size={18} />
          <h2 className="card__title">Watcher Ingestion Settings</h2>
        </div>

        <form onSubmit={handleSaveConfig} className="watcher-config-form">
          <div className="watcher-provider-tabs">
            <button
              type="button"
              className={`provider-tab ${provider === 'webhook' ? 'active' : ''}`}
              onClick={() => setProvider('webhook')}
            >
              <Zap size={16} />
              <div>
                <strong>Webhook / Free Push</strong>
                <span className="provider-tab__tag">$0 Polling (Recommended)</span>
              </div>
            </button>

            <button
              type="button"
              className={`provider-tab ${provider === 'scraper' ? 'active' : ''}`}
              onClick={() => setProvider('scraper')}
            >
              <ShieldCheck size={16} />
              <div>
                <strong>Twitter Scraper API</strong>
                <span className="provider-tab__tag">~$0.0001 / request</span>
              </div>
            </button>

            <button
              type="button"
              className={`provider-tab ${provider === 'grok' ? 'active' : ''}`}
              onClick={() => setProvider('grok')}
            >
              <Radar size={16} />
              <div>
                <strong>xAI Grok Search</strong>
                <span className="provider-tab__tag warning">~$0.35 / search</span>
              </div>
            </button>
          </div>

          {/* Webhook Configuration Subpanel */}
          {provider === 'webhook' && (
            <div className="provider-subpanel animate-fade-in">
              <div className="provider-subpanel__info">
                <HelpCircle size={16} className="text-accent" />
                <p>
                  Zero recurring API cost. Use Zapier, Make, IFTTT, Telegram bots, or a Python script to forward posts to your server's webhook endpoint:
                </p>
              </div>

              <div className="webhook-box">
                <code>{webhookEndpoint}</code>
                <button
                  type="button"
                  className="btn btn-secondary btn-xs"
                  onClick={copyWebhookUrl}
                >
                  {copiedWebhook ? <Check size={14} /> : <Copy size={14} />}
                  {copiedWebhook ? 'Copied!' : 'Copy URL'}
                </button>
              </div>

              <div className="form-group">
                <label htmlFor="webhook-secret">Optional Webhook Secret Token</label>
                <input
                  id="webhook-secret"
                  type="text"
                  className="input"
                  value={webhookSecret}
                  onChange={(e) => setWebhookSecret(e.target.value)}
                  placeholder="Optional secret token for verification"
                />
              </div>
            </div>
          )}

          {/* Scraper Configuration Subpanel */}
          {provider === 'scraper' && (
            <div className="provider-subpanel animate-fade-in">
              <div className="form-grid">
                <div className="form-group">
                  <label htmlFor="scraper-provider">Scraper Engine</label>
                  <select
                    id="scraper-provider"
                    className="input"
                    value={scraperProvider}
                    onChange={(e) =>
                      setScraperProvider(e.target.value as 'twitterapi_io' | 'rapidapi' | 'custom')
                    }
                  >
                    <option value="twitterapi_io">TwitterAPI.io (Free trial / $0.0001/call)</option>
                    <option value="rapidapi">RapidAPI Twitter Timeline</option>
                    <option value="custom">Custom Feed URL / RSS JSON</option>
                  </select>
                </div>

                <div className="form-group">
                  <label htmlFor="scraper-api-key">API Key</label>
                  <input
                    id="scraper-api-key"
                    type="password"
                    className="input"
                    value={scraperApiKey}
                    onChange={(e) => setScraperApiKey(e.target.value)}
                    placeholder="Enter RapidAPI or TwitterAPI.io key"
                  />
                </div>
              </div>

              {scraperProvider === 'custom' && (
                <div className="form-group">
                  <label htmlFor="custom-feed-url">Custom Feed URL (use {'{handle}'} placeholder)</label>
                  <input
                    id="custom-feed-url"
                    type="text"
                    className="input"
                    value={customFeedUrl}
                    onChange={(e) => setCustomFeedUrl(e.target.value)}
                    placeholder="https://my-feed.internal/tweets/{handle}"
                  />
                </div>
              )}
            </div>
          )}

          {/* Grok Search Subpanel */}
          {provider === 'grok' && (
            <div className="provider-subpanel animate-fade-in">
              <div className="alert alert--warning">
                <AlertCircle size={16} />
                <span>
                  <strong>Cost notice:</strong> xAI charges $0.005 per returned post + tool invocation + prompt tokens (~$0.35/call). To minimize costs, use a longer interval like <strong>15m or 30m</strong>.
                </span>
              </div>
            </div>
          )}

          {/* Common Settings: Handles, Interval, Confidence */}
          <div className="form-grid">
            <div className="form-group">
              <label htmlFor="handles">Monitored X Accounts (comma-separated)</label>
              <input
                id="handles"
                type="text"
                className="input"
                value={handlesInput}
                onChange={(e) => setHandlesInput(e.target.value)}
                placeholder="elonmusk, realDonaldTrump"
                required
              />
            </div>

            <div className="form-group">
              <label htmlFor="interval">Scan Frequency Interval</label>
              <select
                id="interval"
                className="input"
                value={intervalSecs}
                onChange={(e) => setIntervalSecs(Number(e.target.value))}
              >
                <option value={60}>1 minute (Grok: ~1,440 checks/day — High cost)</option>
                <option value={300}>5 minutes (288 checks/day)</option>
                <option value={600}>10 minutes (144 checks/day)</option>
                <option value={900}>15 minutes (96 checks/day — Recommended)</option>
                <option value={1800}>30 minutes (48 checks/day)</option>
                <option value={3600}>1 hour (24 checks/day)</option>
              </select>
            </div>

            <div className="form-group">
              <label htmlFor="min-confidence">
                Min Confidence Threshold: <strong>{Math.round(minConfidence * 100)}%</strong>
              </label>
              <input
                id="min-confidence"
                type="range"
                min="0.5"
                max="0.95"
                step="0.05"
                value={minConfidence}
                onChange={(e) => setMinConfidence(Number(e.target.value))}
                className="range-input"
              />
            </div>

            <div className="form-group form-group--toggle">
              <label htmlFor="enabled-toggle">Watcher State</label>
              <div className="toggle-wrap">
                <button
                  type="button"
                  id="enabled-toggle"
                  className={`btn ${enabled ? 'btn-success' : 'btn-secondary'} btn-sm`}
                  onClick={() => setEnabled(!enabled)}
                >
                  {enabled ? <Play size={14} /> : <Pause size={14} />}
                  {enabled ? 'Running' : 'Paused'}
                </button>
              </div>
            </div>
          </div>

          <div className="watcher-form-actions">
            <button type="submit" className="btn btn-primary" disabled={isSaving}>
              {isSaving ? 'Saving...' : 'Save Watcher Settings'}
            </button>
          </div>
        </form>
      </section>

      {/* Posts Seen Table */}
      <section className="signals-page__section">
        <div className="signals-page__section-header">
          <h2 className="signals-page__section-title">Ingested Posts & Review History</h2>
          <span className="text-muted text-sm">{watch?.posts?.length || 0} recent posts cached in database</span>
        </div>

        {watch && watch.posts.length > 0 ? (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Timestamp</th>
                  <th>Account</th>
                  <th>Post Content</th>
                  <th>Signal Status</th>
                </tr>
              </thead>
              <tbody>
                {watch.posts.map((post) => (
                  <tr key={post.post_id}>
                    <td className="text-muted text-sm whitespace-nowrap">
                      {new Date(post.seen_at).toLocaleString()}
                    </td>
                    <td>
                      <span className="badge badge--secondary">@{post.handle}</span>
                    </td>
                    <td className="watcher-page__body">{post.body}</td>
                    <td>
                      {post.queued ? (
                        <span className="badge badge--success">Queued for Review</span>
                      ) : (
                        <span className="badge badge--secondary">Filtered / Ignored</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="signals-page__empty card">
            <p>No posts recorded in database yet</p>
            <p className="text-muted">
              Posts will appear here as soon as incoming webhooks, scraper scans, or test posts are ingested.
            </p>
          </div>
        )}
      </section>
    </div>
  );
}
