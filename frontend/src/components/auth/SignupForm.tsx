import React from 'react';
import { Lock, User as UserIcon, Mail, ArrowRight, CheckCircle2, Eye, EyeOff, ShieldCheck, Activity } from 'lucide-react';
import { AuthTab, Setter } from './types';

interface SignupFormProps {
  setTab: Setter<AuthTab>;
  signupUsername: string;
  setSignupUsername: Setter<string>;
  signupEmail: string;
  setSignupEmail: Setter<string>;
  signupPassword: string;
  setSignupPassword: Setter<string>;
  signupConfirm: string;
  setSignupConfirm: Setter<string>;
  showSignupPassword: boolean;
  setShowSignupPassword: Setter<boolean>;
  loading: boolean;
  setError: Setter<string | null>;
  allowSignup: boolean;
  handleSignup: (e: React.FormEvent) => void;
}

/** Account creation form, or a notice when public signup is disabled. */
export const SignupForm: React.FC<SignupFormProps> = ({
  setTab,
  signupUsername,
  setSignupUsername,
  signupEmail,
  setSignupEmail,
  signupPassword,
  setSignupPassword,
  signupConfirm,
  setSignupConfirm,
  showSignupPassword,
  setShowSignupPassword,
  loading,
  setError,
  allowSignup,
  handleSignup,
}) => (
    !allowSignup ? (
      <div className="text-center py-6 space-y-3">
        <div className="w-12 h-12 bg-amber-50 border border-amber-200 text-amber-700 rounded-2xl mx-auto flex items-center justify-center shadow-xs">
          <ShieldCheck className="w-6 h-6" />
        </div>
        <h4 className="text-sm font-bold text-slate-900">Registration Disabled</h4>
        <p className="text-xs text-slate-500 max-w-xs mx-auto leading-relaxed">
          Public account registration is disabled on this server. Please contact your administrator for access.
        </p>
        <div className="pt-2">
          <button
            type="button"
            onClick={() => setTab('login')}
            className="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs font-semibold shadow-xs transition"
          >
            Return to Sign In
          </button>
        </div>
      </div>
    ) : (
    <form onSubmit={handleSignup} className="space-y-3.5">
      <div>
        <label className="block text-xs font-semibold text-slate-700 mb-1">
          Username *
        </label>
        <div className="relative">
          <UserIcon className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            type="text"
            required
            autoComplete="username"
            placeholder="At least 3 characters"
            value={signupUsername}
            onChange={(e) => setSignupUsername(e.target.value)}
            className="w-full pl-9 pr-3 py-2 text-xs sm:text-sm text-slate-900 placeholder:text-slate-400 bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 focus:bg-white font-medium transition"
          />
        </div>
      </div>

      <div>
        <label className="block text-xs font-semibold text-slate-700 mb-1">
          Email Address *
        </label>
        <div className="relative">
          <Mail className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            type="email"
            required
            autoComplete="email"
            placeholder="name@example.com"
            value={signupEmail}
            onChange={(e) => setSignupEmail(e.target.value)}
            className="w-full pl-9 pr-3 py-2 text-xs sm:text-sm text-slate-900 placeholder:text-slate-400 bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 focus:bg-white font-medium transition"
          />
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div>
          <label className="block text-xs font-semibold text-slate-700 mb-1">
            Password *
          </label>
          <div className="relative">
            <Lock className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type={showSignupPassword ? 'text' : 'password'}
              required
              autoComplete="new-password"
              placeholder="6+ chars"
              value={signupPassword}
              onChange={(e) => setSignupPassword(e.target.value)}
              className="w-full pl-9 pr-8 py-2 text-xs sm:text-sm text-slate-900 placeholder:text-slate-400 bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 focus:bg-white font-mono transition"
            />
            <button
              type="button"
              onClick={() => setShowSignupPassword(!showSignupPassword)}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600 transition"
              tabIndex={-1}
            >
              {showSignupPassword ? (
                <EyeOff className="w-3.5 h-3.5" />
              ) : (
                <Eye className="w-3.5 h-3.5" />
              )}
            </button>
          </div>
        </div>

        <div>
          <label className="block text-xs font-semibold text-slate-700 mb-1">
            Confirm Password *
          </label>
          <div className="relative">
            <Lock className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type={showSignupPassword ? 'text' : 'password'}
              required
              autoComplete="new-password"
              placeholder="Repeat password"
              value={signupConfirm}
              onChange={(e) => setSignupConfirm(e.target.value)}
              className="w-full pl-9 pr-3 py-2 text-xs sm:text-sm text-slate-900 placeholder:text-slate-400 bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 focus:bg-white font-mono transition"
            />
          </div>
        </div>
      </div>

      <div className="p-3 bg-emerald-50/80 border border-emerald-100 rounded-lg flex items-start space-x-2 text-[11px] text-emerald-800">
        <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0 mt-0.5" />
        <span className="leading-relaxed">
          Your ledger is initialized with standard GnuCash accounts (Assets, Liabilities, Equity, Income, Expenses).
        </span>
      </div>

      <button
        type="submit"
        disabled={loading}
        className="w-full mt-2 py-2.5 px-4 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs sm:text-sm font-semibold transition shadow-md shadow-emerald-700/20 disabled:opacity-50 flex items-center justify-center space-x-2 active:scale-[0.99]"
      >
        {loading ? (
          <>
            <Activity className="w-4 h-4 animate-spin" />
            <span>Creating account...</span>
          </>
        ) : (
          <>
            <span>Create Account & Sign In</span>
            <ArrowRight className="w-4 h-4" />
          </>
        )}
      </button>

      <div className="text-center pt-2.5 border-t border-slate-100">
        <p className="text-xs text-slate-500">
          Already have an account?{' '}
          <button
            type="button"
            onClick={() => {
              setTab('login');
              setError(null);
            }}
            className="font-bold text-emerald-600 hover:text-emerald-700 hover:underline"
          >
            Sign in
          </button>
        </p>
      </div>
    </form>
  )
);
