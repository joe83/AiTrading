import { useCallback, useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { Radar } from 'lucide-react';
import { api, type GrokAccount, type WatchSnapshot } from '../api/client';
import './WatcherPage.css';

export function WatcherPage() {
  const [watch, setWatch] = useState<WatchSnapshot | null>(null);
  const [account, setAccount] = useState<GrokAccount | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [nextWatch, nextAccount] = await Promise.all([api.getWatch(), api.getGrokAccount()]);
      setWatch(nextWatch);
      setAccount(nextAccount);
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

  const status = watch?.status;

  return (
    <div className="page watcher-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">Watcher</h1>
        <p className="page__subtitle">
          Scans X for new posts and queues a review signal. It does not place an order.
        </p>
      </div>

      {error && <p className="watcher-page__error" role="alert">{error}</p>}

      <section className="watcher-page__account card">
        <div>
          <h2 className="card__title">Grok connection</h2>
          <p className="watcher-page__account-name">
            {account?.logged_in ? `SuperGrok · ${account.account}` : 'xAI API key'}
          </p>
        </div>
        <Link to="/api-keys" className="btn btn-secondary">API Keys</Link>
      </section>

      <section className="watcher-page__status card">
        <div className="watcher-page__status-head">
          <Radar size={18} />
          <h2 className="card__title">{status?.running ? 'Scanning' : 'Stopped'}</h2>
        </div>
        <p>{status?.last_result || 'No scan yet'}</p>
        <p className="text-muted">
          {status?.handles?.length ? status.handles.map((handle) => `@${handle}`).join(', ') : 'No accounts configured'}
          {status?.interval_secs ? ` · every ${status.interval_secs}s` : ''}
        </p>
        {status?.last_tick_at && (
          <p className="text-muted">Last scan {new Date(status.last_tick_at).toLocaleString()}</p>
        )}
        {status?.last_error && <p className="watcher-page__error">{status.last_error}</p>}
      </section>

      <section className="signals-page__section">
        <h2 className="signals-page__section-title">Posts seen</h2>
        {watch && watch.posts.length > 0 ? (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>When</th>
                  <th>Account</th>
                  <th>Post</th>
                  <th>Queued</th>
                </tr>
              </thead>
              <tbody>
                {watch.posts.map((post) => (
                  <tr key={post.post_id}>
                    <td className="text-muted">{new Date(post.seen_at).toLocaleString()}</td>
                    <td>@{post.handle}</td>
                    <td className="watcher-page__body">{post.body}</td>
                    <td>{post.queued ? 'Yes' : 'No'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="signals-page__empty card">
            <p>No posts recorded yet</p>
            <p className="text-muted">A quiet scan still counts. Posts show up here after the next pass.</p>
          </div>
        )}
      </section>
    </div>
  );
}
