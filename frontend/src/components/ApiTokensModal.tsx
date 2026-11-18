import React, { useCallback, useEffect, useState } from 'react';
import {
  AlertCircle,
  Check,
  CheckCircle2,
  Copy,
  KeyRound,
  Plus,
  ShieldCheck,
  Trash2,
  X,
} from 'lucide-react';
import { createApiToken, fetchApiTokens, getErrorMessage, revokeApiToken } from '../api';
import { ApiToken, CreatedApiToken } from '../types';

interface ApiTokensModalProps {
  isOpen: boolean;
  onClose: () => void;
  username: string;
}

const EXPIRY_OPTIONS = [
  { value: '', label: 'Never expires' },
  { value: '30', label: '30 days' },
  { value: '90', label: '90 days' },
  { value: '365', label: '1 year' },
];

function tokenStatus(token: ApiToken): { label: string; className: string } {
  if (token.revoked_at) {
    return { label: 'Revoked', className: 'bg-rose-50 text-rose-700 border-rose-200' };
  }
  if (token.expires_at && new Date(token.expires_at).getTime() <= Date.now()) {
    return { label: 'Expired', className: 'bg-amber-50 text-amber-700 border-amber-200' };
  }
  return { label: 'Active', className: 'bg-emerald-50 text-emerald-700 border-emerald-200' };
}

function formatDate(value: string | null): string {
  if (!value) return '—';
  return new Date(value).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/** API token management for service clients (UI-8, SEC-16 to SEC-20). */
export const ApiTokensModal: React.FC<ApiTokensModalProps> = ({
  isOpen,
  onClose,
  username,
}) => {
  const [tokens, setTokens] = useState<ApiToken[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [name, setName] = useState('');
  const [expiry, setExpiry] = useState('');
  const [creating, setCreating] = useState(false);
  const [created, setCreated] = useState<CreatedApiToken | null>(null);
  const [copied, setCopied] = useState(false);

  const [confirmRevokeId, setConfirmRevokeId] = useState<string | null>(null);
  const [revokingId, setRevokingId] = useState<string | null>(null);

  const loadTokens = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setTokens(await fetchApiTokens());
    } catch (err) {
      setError(getErrorMessage(err) || 'Failed to load API tokens');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      setCreated(null);
      setCopied(false);
      setConfirmRevokeId(null);
      setName('');
      setExpiry('');
      void loadTokens();
    }
  }, [isOpen, loadTokens]);

  if (!isOpen) return null;

  const handleClose = () => {
    if (creating || revokingId) return;
    setError(null);
    onClose();
  };

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    const trimmed = name.trim();
    if (!trimmed) {
      setError('Please give the token a name (for example, sms_webhook).');
      return;
    }

    try {
      setCreating(true);
      const token = await createApiToken({
        name: trimmed,
        expires_in_days: expiry ? Number(expiry) : null,
      });
      setCreated(token);
      setCopied(false);
      setName('');
      setExpiry('');
      await loadTokens();
    } catch (err) {
      setError(getErrorMessage(err) || 'Failed to create API token');
    } finally {
      setCreating(false);
    }
  };

  const copyTokenToClipboard = (token: string): boolean => {
    try {
      const el = document.createElement('textarea');
      el.value = token;
      el.setAttribute('readonly', '');
      el.style.position = 'fixed';
      el.style.opacity = '0';
      document.body.appendChild(el);
      el.select();
      const copied = document.execCommand('copy');
      el.remove();
      return copied;
    } catch {
      return false;
    }
  };

  const handleCopy = async () => {
    if (!created) return;
    const token = created.token;

    // Prefer the synchronous textarea copy: it runs inside the click gesture, so browsers that
    // block (or never resolve) the async Clipboard API in some environments still work.
    if (copyTokenToClipboard(token)) {
      setCopied(true);
      return;
    }

    try {
      if (navigator.clipboard?.writeText) {
        await Promise.race([
          navigator.clipboard.writeText(token),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error('Clipboard write timed out')), 1000)
          ),
        ]);
        setCopied(true);
        return;
      }
    } catch {
      // Fall through to the manual-copy message.
    }
    setError('Could not copy the token. Select it and copy manually.');
  };

  const handleRevoke = async (id: string) => {
    setError(null);
    try {
      setRevokingId(id);
      await revokeApiToken(id);
      setConfirmRevokeId(null);
      if (created?.id === id) {
        setCreated(null);
      }
      await loadTokens();
    } catch (err) {
      setError(getErrorMessage(err) || 'Failed to revoke API token');
    } finally {
      setRevokingId(null);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/60 backdrop-blur-xs animate-in fade-in duration-150"
      data-testid="api-tokens-modal"
    >
      <div
        className="relative w-full max-w-2xl max-h-[90vh] overflow-y-auto bg-white rounded-2xl shadow-2xl border border-slate-200"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-100 bg-slate-50/70">
          <div className="flex items-center space-x-2.5">
            <div className="w-9 h-9 rounded-xl bg-emerald-100 text-emerald-700 flex items-center justify-center shadow-xs">
              <KeyRound className="w-5 h-5" />
            </div>
            <div>
              <h3 className="text-sm font-bold text-slate-900">API Tokens</h3>
              <p className="text-xs text-slate-500">
                Service access for <span className="font-semibold text-slate-700">{username}</span>
              </p>
            </div>
          </div>
          <button
            onClick={handleClose}
            type="button"
            aria-label="Close API Tokens"
            data-testid="api-token-close"
            className="p-1 text-slate-400 hover:text-slate-600 rounded-lg hover:bg-slate-100 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="p-5 sm:p-6 space-y-5">
          {error && (
            <div className="p-3 bg-rose-50 border border-rose-200 rounded-xl text-xs text-rose-700 flex items-center space-x-2">
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-500" />
              <span data-testid="api-token-error">{error}</span>
            </div>
          )}

          {/* Shown-once token */}
          {created && (
            <div className="p-4 bg-emerald-50 border border-emerald-200 rounded-xl space-y-2">
              <div className="flex items-center space-x-2 text-xs font-semibold text-emerald-800">
                <CheckCircle2 className="w-4 h-4 text-emerald-600" />
                <span>Token created — copy it now</span>
              </div>
              <div className="flex items-center space-x-2">
                <code
                  data-testid="api-token-value"
                  className="flex-1 min-w-0 truncate px-3 py-2 bg-white border border-emerald-200 rounded-lg text-xs font-mono text-slate-800"
                >
                  {created.token}
                </code>
                <button
                  type="button"
                  onClick={handleCopy}
                  data-testid="api-token-copy"
                  className="flex items-center space-x-1 px-2.5 py-2 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg transition shrink-0"
                >
                  {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                  <span>{copied ? 'Copied' : 'Copy'}</span>
                </button>
              </div>
              <p className="text-[11px] text-emerald-800">
                You will not see this token again. Store it in the service configuration now.
              </p>
              <button
                type="button"
                onClick={() => setCreated(null)}
                className="text-[11px] font-semibold text-emerald-700 hover:text-emerald-900 underline"
              >
                I have saved it
              </button>
            </div>
          )}

          {/* Create form */}
          <form onSubmit={handleCreate} className="space-y-3">
            <div className="flex items-center space-x-2 text-xs font-bold text-slate-700 uppercase tracking-wider">
              <Plus className="w-4 h-4 text-emerald-600" />
              <span>New token</span>
            </div>
            <div className="flex flex-col sm:flex-row gap-2">
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                maxLength={100}
                placeholder="Name (e.g. sms_webhook)"
                aria-label="Token name"
                data-testid="api-token-name"
                className="flex-1 px-3 py-2 text-xs sm:text-sm bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
              />
              <select
                value={expiry}
                onChange={(e) => setExpiry(e.target.value)}
                aria-label="Token expiry"
                data-testid="api-token-expiry"
                className="px-3 py-2 text-xs sm:text-sm bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 cursor-pointer"
              >
                {EXPIRY_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
              <button
                type="submit"
                disabled={creating}
                data-testid="api-token-create"
                className="px-4 py-2 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg transition shadow-sm disabled:opacity-50"
              >
                {creating ? 'Creating...' : 'Create Token'}
              </button>
            </div>
          </form>

          {/* Token list */}
          <div className="space-y-2">
            <div className="flex items-center space-x-2 text-xs font-bold text-slate-700 uppercase tracking-wider">
              <ShieldCheck className="w-4 h-4 text-emerald-600" />
              <span>Your tokens</span>
            </div>
            {loading && <p className="text-xs text-slate-500">Loading tokens…</p>}
            {!loading && tokens.length === 0 && (
              <p className="text-xs text-slate-500" data-testid="api-token-empty">
                No API tokens yet. Create one to let a service post transactions.
              </p>
            )}
            {!loading && tokens.length > 0 && (
              <div className="border border-slate-200 rounded-xl overflow-hidden">
                <div className="hidden sm:grid grid-cols-[1.4fr_1fr_1fr_1fr_auto] gap-2 px-3 py-2 bg-slate-50 text-[10px] font-bold text-slate-500 uppercase tracking-wider border-b border-slate-200">
                  <span>Name / prefix</span>
                  <span>Created</span>
                  <span>Last used</span>
                  <span>Expires</span>
                  <span className="text-right">Status</span>
                </div>
                {tokens.map((token) => {
                  const status = tokenStatus(token);
                  const isRevoked = Boolean(token.revoked_at);
                  return (
                    <div
                      key={token.id}
                      data-testid="api-token-row"
                      data-token-name={token.name}
                      data-token-status={status.label}
                      className="grid grid-cols-1 sm:grid-cols-[1.4fr_1fr_1fr_1fr_auto] gap-1 sm:gap-2 px-3 py-2.5 border-b border-slate-100 last:border-b-0 items-center"
                    >
                      <div className="min-w-0">
                        <p className="text-xs font-semibold text-slate-800 truncate">{token.name}</p>
                        <p className="text-[10px] font-mono text-slate-500 truncate">
                          {token.token_prefix}…
                        </p>
                      </div>
                      <span className="text-[11px] text-slate-600">
                        <span className="sm:hidden font-semibold text-slate-500">Created: </span>
                        {formatDate(token.created_at)}
                      </span>
                      <span className="text-[11px] text-slate-600">
                        <span className="sm:hidden font-semibold text-slate-500">Last used: </span>
                        {formatDate(token.last_used_at)}
                      </span>
                      <span className="text-[11px] text-slate-600">
                        <span className="sm:hidden font-semibold text-slate-500">Expires: </span>
                        {formatDate(token.expires_at)}
                      </span>
                      <div className="flex items-center justify-end gap-2">
                        <span
                          className={`text-[10px] font-semibold px-2 py-0.5 rounded-full border ${status.className}`}
                        >
                          {status.label}
                        </span>
                        {!isRevoked &&
                          (confirmRevokeId === token.id ? (
                            <span className="flex items-center gap-1">
                              <button
                                type="button"
                                onClick={() => handleRevoke(token.id)}
                                disabled={revokingId === token.id}
                                data-testid="api-token-confirm-revoke"
                                className="px-2 py-1 text-[10px] font-semibold bg-rose-600 hover:bg-rose-500 text-white rounded-md transition disabled:opacity-50"
                              >
                                {revokingId === token.id ? 'Revoking…' : 'Confirm'}
                              </button>
                              <button
                                type="button"
                                onClick={() => setConfirmRevokeId(null)}
                                className="px-2 py-1 text-[10px] font-medium text-slate-600 hover:bg-slate-100 rounded-md transition"
                              >
                                Cancel
                              </button>
                            </span>
                          ) : (
                            <button
                              type="button"
                              onClick={() => setConfirmRevokeId(token.id)}
                              aria-label={`Revoke ${token.name}`}
                              data-testid="api-token-revoke"
                              className="p-1.5 text-rose-500 hover:text-rose-700 hover:bg-rose-50 rounded-md transition"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          ))}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
