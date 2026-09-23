import { useEffect, useRef, useState } from 'react';
import { Check, Loader2, Send, Square, X } from 'lucide-react';
import { api, type LessonRecord } from '../api/client';
import './AgentPage.css';

const API_BASE = import.meta.env.VITE_API_URL ?? '';

interface TranscriptItem {
  id: string;
  kind: 'user' | 'assistant' | 'tool';
  text: string;
  ok?: boolean;
  pending?: boolean;
}

interface Proposal {
  id: string;
  scope: string;
  claim: string;
  sampleSize: number;
  status: 'hypothesis' | 'supported' | 'rejected';
}

const STARTERS = [
  'What do my closed trades have in common?',
  'Which symbols have the worst win rate?',
  'Look for a repeating loss and save it as a lesson.',
];

export function AgentPage() {
  const [items, setItems] = useState<TranscriptItem[]>([]);
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [draft, setDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actingId, setActingId] = useState<string | null>(null);
  const logRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);
  const nextId = useRef(1);

  useEffect(() => {
    api.getLessons('hypothesis')
      .then((response) => {
        setProposals(response.lessons.map(toProposal));
      })
      .catch(() => {
        setError('Could not load saved lessons.');
      });
    return () => abortRef.current?.abort();
  }, []);

  useEffect(() => {
    const log = logRef.current;
    if (log) log.scrollTop = log.scrollHeight;
  }, [items, busy]);

  async function send(text: string) {
    const content = text.trim();
    if (!content || busy) return;

    const history = [
      ...items
        .filter((item) => item.kind === 'user' || item.kind === 'assistant')
        .map((item) => ({ role: item.kind, content: item.text })),
      { role: 'user' as const, content },
    ];

    setItems((current) => [...current, { id: makeId(), kind: 'user', text: content }]);
    setDraft('');
    setBusy(true);
    setError(null);

    const controller = new AbortController();
    abortRef.current = controller;

    try {
      const token = localStorage.getItem('aitrading_token');
      const response = await fetch(`${API_BASE}/api/agent/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: JSON.stringify({ messages: history }),
        signal: controller.signal,
      });

      if (response.status === 401) {
        window.location.href = '/login';
        return;
      }
      if (!response.ok || !response.body) {
        throw new Error(`The agent request failed (${response.status}).`);
      }

      await readStream(response.body, {
        onToolStart: (name) => {
          setItems((current) => [
            ...current,
            { id: makeId(), kind: 'tool', text: name, pending: true },
          ]);
        },
        onToolResult: (event) => {
          setItems((current) => {
            const next = [...current];
            const index = [...next].reverse().findIndex((item) => item.kind === 'tool' && item.pending);
            if (index === -1) return next;
            const target = next.length - 1 - index;
            next[target] = {
              ...next[target],
              text: `${event.name}: ${event.summary || next[target].text}`,
              ok: event.ok,
              pending: false,
            };
            return next;
          });
          const lesson = lessonFromResult(event.name, event.result);
          if (lesson) {
            setProposals((current) => [lesson, ...current.filter((item) => item.id !== lesson.id)]);
          }
        },
        onMessage: (content) => {
          setItems((current) => [...current, { id: makeId(), kind: 'assistant', text: content }]);
        },
        onError: (message) => setError(message),
      });
    } catch (err) {
      if ((err as Error).name !== 'AbortError') {
        setError(err instanceof Error ? err.message : 'The agent request failed.');
      }
    } finally {
      setBusy(false);
      abortRef.current = null;
    }
  }

  async function decide(id: string, action: 'approve' | 'reject') {
    setActingId(id);
    setError(null);
    try {
      if (action === 'approve') {
        await api.approveLesson(id);
        setProposals((current) =>
          current.map((item) => (item.id === id ? { ...item, status: 'supported' } : item)),
        );
      } else {
        await api.rejectLesson(id);
        setProposals((current) =>
          current.map((item) => (item.id === id ? { ...item, status: 'rejected' } : item)),
        );
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not update the lesson.');
    } finally {
      setActingId(null);
    }
  }

  function makeId() {
    nextId.current += 1;
    return `m${nextId.current}`;
  }

  return (
    <div className="page agent-page animate-fade-in">
      <div className="page__header">
        <h1 className="page__title">Review agent</h1>
        <p className="page__subtitle">
          Ask Grok about closed trades. A saved lesson stays a hypothesis until you approve it.
        </p>
      </div>

      {error && <p className="agent-page__error" role="alert">{error}</p>}

      <div className="agent-page__layout">
        <section className="agent-page__thread card">
          <div className="agent-page__log" ref={logRef}>
            {items.length === 0 && (
              <div className="agent-page__empty">
                <p>Ask about a pattern in the journal, then turn a finding into a lesson.</p>
                <div className="agent-page__starters">
                  {STARTERS.map((prompt) => (
                    <button key={prompt} type="button" className="btn btn-ghost" onClick={() => send(prompt)} disabled={busy}>
                      {prompt}
                    </button>
                  ))}
                </div>
              </div>
            )}
            {items.map((item) => (
              <div key={item.id} className={`agent-page__item agent-page__item--${item.kind}`}>
                {item.kind === 'tool' ? (
                  <span className={`agent-page__chip ${item.ok === false ? 'agent-page__chip--bad' : ''}`}>
                    {item.pending && <Loader2 size={14} className="agent-page__spin" />}
                    {item.text}
                  </span>
                ) : (
                  <p>{item.text}</p>
                )}
              </div>
            ))}
          </div>

          <form
            className="agent-page__composer"
            onSubmit={(event) => {
              event.preventDefault();
              void send(draft);
            }}
          >
            <textarea
              className="agent-page__input"
              value={draft}
              placeholder="Ask about a symbol, a losing streak, or a rule to test."
              rows={2}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && !event.shiftKey) {
                  event.preventDefault();
                  void send(draft);
                }
              }}
            />
            {busy ? (
              <button type="button" className="btn btn-ghost" onClick={() => abortRef.current?.abort()}>
                <Square size={16} /> Stop
              </button>
            ) : (
              <button type="submit" className="btn btn-primary" disabled={!draft.trim()}>
                <Send size={16} /> Send
              </button>
            )}
          </form>
        </section>

        <aside className="agent-page__panel card">
          <h2 className="card__title">Lessons</h2>
          <p className="agent-page__panel-note">
            Approve adds the claim to the playbook the next Grok decision reads. Reject keeps it out.
          </p>
          {proposals.length === 0 ? (
            <p className="text-muted agent-page__panel-empty">No hypotheses yet.</p>
          ) : (
            <ul className="agent-page__lessons">
              {proposals.map((lesson) => (
                <li key={lesson.id} className={`agent-page__lesson agent-page__lesson--${lesson.status}`}>
                  <div className="agent-page__lesson-meta">
                    <span>{lesson.scope}</span>
                    <span>{lesson.status}</span>
                  </div>
                  <p>{lesson.claim}</p>
                  {lesson.sampleSize > 0 && (
                    <span className="agent-page__sample">Sample {lesson.sampleSize}</span>
                  )}
                  {lesson.status === 'hypothesis' && (
                    <div className="agent-page__lesson-actions">
                      <button
                        type="button"
                        className="btn btn-success"
                        disabled={actingId === lesson.id}
                        onClick={() => void decide(lesson.id, 'approve')}
                      >
                        <Check size={14} /> Approve
                      </button>
                      <button
                        type="button"
                        className="btn btn-danger"
                        disabled={actingId === lesson.id}
                        onClick={() => void decide(lesson.id, 'reject')}
                      >
                        <X size={14} /> Reject
                      </button>
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}
          <p className="agent-page__mcp">
            The same tools are available to an external client at <code>POST /api/mcp</code> with this session’s bearer token.
          </p>
        </aside>
      </div>
    </div>
  );
}

function toProposal(lesson: LessonRecord): Proposal {
  const status = lesson.status === 'supported' || lesson.status === 'rejected' ? lesson.status : 'hypothesis';
  return {
    id: lesson.id,
    scope: lesson.scope,
    claim: lesson.claim,
    sampleSize: lesson.sample_size,
    status,
  };
}

function lessonFromResult(name: string, result: Record<string, unknown> | undefined): Proposal | null {
  if (name !== 'save_lesson' || !result || typeof result.id !== 'string') return null;
  return {
    id: result.id,
    scope: typeof result.scope === 'string' ? result.scope : 'global',
    claim: typeof result.claim === 'string' ? result.claim : 'Saved hypothesis',
    sampleSize: typeof result.sample_size === 'number' ? result.sample_size : 0,
    status: 'hypothesis',
  };
}

interface ToolResultEvent {
  name: string;
  ok: boolean;
  summary: string;
  result?: Record<string, unknown>;
}

async function readStream(
  body: ReadableStream<Uint8Array>,
  handlers: {
    onToolStart: (name: string) => void;
    onToolResult: (event: ToolResultEvent) => void;
    onMessage: (content: string) => void;
    onError: (message: string) => void;
  },
) {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const chunks = buffer.split('\n\n');
    buffer = chunks.pop() ?? '';
    for (const chunk of chunks) {
      dispatchSse(chunk, handlers);
    }
  }
  if (buffer.trim()) dispatchSse(buffer, handlers);
}

function dispatchSse(
  chunk: string,
  handlers: {
    onToolStart: (name: string) => void;
    onToolResult: (event: ToolResultEvent) => void;
    onMessage: (content: string) => void;
    onError: (message: string) => void;
  },
) {
  let event = 'message';
  const data: string[] = [];
  for (const line of chunk.split('\n')) {
    if (line.startsWith(':')) continue;
    if (line.startsWith('event:')) event = line.slice(6).trim();
    else if (line.startsWith('data:')) data.push(line.slice(5).trim());
  }
  if (data.length === 0) return;
  let payload: Record<string, unknown> = {};
  try {
    payload = JSON.parse(data.join('\n')) as Record<string, unknown>;
  } catch {
    return;
  }

  if (event === 'tool_start' && typeof payload.name === 'string') {
    handlers.onToolStart(payload.name);
  } else if (event === 'tool_result') {
    handlers.onToolResult({
      name: typeof payload.name === 'string' ? payload.name : 'tool',
      ok: payload.ok !== false,
      summary: typeof payload.summary === 'string' ? payload.summary : 'done',
      result: payload.result && typeof payload.result === 'object' ? payload.result as Record<string, unknown> : undefined,
    });
  } else if (event === 'message' && typeof payload.content === 'string') {
    handlers.onMessage(payload.content);
  } else if (event === 'error') {
    handlers.onError(typeof payload.message === 'string' ? payload.message : 'The agent failed.');
  }
}
