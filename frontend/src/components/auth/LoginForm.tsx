import React from 'react';
import { Lock, User as UserIcon, ArrowRight, Eye, EyeOff, Activity } from 'lucide-react';
import { AuthTab, Setter } from './types';

interface LoginFormProps {
  setTab: Setter<AuthTab>;
  loginUsername: string;
  setLoginUsername: Setter<string>;
  loginPassword: string;
  setLoginPassword: Setter<string>;
  showLoginPassword: boolean;
  setShowLoginPassword: Setter<boolean>;
  loading: boolean;
  setError: Setter<string | null>;
  allowSignup: boolean;
  handleLogin: (e?: React.FormEvent) => void;
}

/** Username/email and password sign-in form. */
export const LoginForm: React.FC<LoginFormProps> = ({
  setTab,
  loginUsername,
  setLoginUsername,
  loginPassword,
  setLoginPassword,
  showLoginPassword,
  setShowLoginPassword,
  loading,
  setError,
  allowSignup,
  handleLogin,
}) => (
  <form onSubmit={handleLogin} className="space-y-4">
    <div>
      <label className="block text-xs font-semibold text-slate-700 mb-1.5">
        Username or Email
      </label>
      <div className="relative">
        <UserIcon className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
        <input
          type="text"
          required
          autoComplete="username"
          placeholder="Enter your username or email"
          value={loginUsername}
          onChange={(e) => setLoginUsername(e.target.value)}
          className="w-full pl-9 pr-3 py-2.5 text-xs sm:text-sm text-slate-900 placeholder:text-slate-400 bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 focus:bg-white font-medium transition"
        />
      </div>
    </div>

    <div>
      <div className="flex items-center justify-between mb-1.5">
        <label className="block text-xs font-semibold text-slate-700">
          Password
        </label>
        <button
          type="button"
          onClick={() => {
            setTab('reset');
            setError(null);
          }}
          className="text-[11px] text-emerald-600 hover:text-emerald-700 hover:underline font-semibold"
        >
          Forgot password?
        </button>
      </div>
      <div className="relative">
        <Lock className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
        <input
          type={showLoginPassword ? 'text' : 'password'}
          required
          autoComplete="current-password"
          placeholder="••••••••"
          value={loginPassword}
          onChange={(e) => setLoginPassword(e.target.value)}
          className="w-full pl-9 pr-10 py-2.5 text-xs sm:text-sm text-slate-900 placeholder:text-slate-400 bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 focus:bg-white font-mono transition"
        />
        <button
          type="button"
          onClick={() => setShowLoginPassword(!showLoginPassword)}
          className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600 transition p-0.5"
          tabIndex={-1}
        >
          {showLoginPassword ? (
            <EyeOff className="w-4 h-4" />
          ) : (
            <Eye className="w-4 h-4" />
          )}
        </button>
      </div>
    </div>

    <button
      type="submit"
      disabled={loading}
      className="w-full mt-2 py-2.5 px-4 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs sm:text-sm font-semibold transition shadow-md shadow-emerald-700/20 disabled:opacity-50 flex items-center justify-center space-x-2 active:scale-[0.99]"
    >
      {loading ? (
        <>
          <Activity className="w-4 h-4 animate-spin" />
          <span>Signing in...</span>
        </>
      ) : (
        <>
          <span>Sign In</span>
          <ArrowRight className="w-4 h-4" />
        </>
      )}
    </button>

    {allowSignup && (
      <div className="text-center pt-3 border-t border-slate-100">
        <p className="text-xs text-slate-500">
          Don't have an account?{' '}
          <button
            type="button"
            onClick={() => {
              setTab('signup');
              setError(null);
            }}
            className="font-bold text-emerald-600 hover:text-emerald-700 hover:underline"
          >
            Create an account
          </button>
        </p>
      </div>
    )}
  </form>

);
