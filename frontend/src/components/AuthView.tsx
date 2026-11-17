import React, { useState, useEffect } from 'react';
import {
  Landmark,
  Lock,
  User as UserIcon,
  Mail,
  AlertCircle,
  ArrowRight,
  CheckCircle2,
  Eye,
  EyeOff,
  KeyRound,
  Coins,
  Scale,
  TrendingUp,
  Layers,
  ShieldCheck,
  Sparkles,
  Check,
  Activity,
} from 'lucide-react';
import { login, signup, checkHealth, getErrorMessage } from '../api';
import { User } from '../types';

interface AuthViewProps {
  onAuthSuccess: (user: User) => void;
}

export const AuthView: React.FC<AuthViewProps> = ({ onAuthSuccess }) => {
  const [tab, setTab] = useState<'login' | 'signup' | 'reset'>('login');

  // Login form state
  const [loginUsername, setLoginUsername] = useState('');
  const [loginPassword, setLoginPassword] = useState('');
  const [showLoginPassword, setShowLoginPassword] = useState(false);

  // Signup form state
  const [signupUsername, setSignupUsername] = useState('');
  const [signupEmail, setSignupEmail] = useState('');
  const [signupPassword, setSignupPassword] = useState('');
  const [signupConfirm, setSignupConfirm] = useState('');
  const [showSignupPassword, setShowSignupPassword] = useState(false);


  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);
  const [backendOnline, setBackendOnline] = useState<boolean | null>(null);
  const [allowSignup, setAllowSignup] = useState<boolean>(false);

  // Check backend health on mount
  useEffect(() => {
    checkHealth()
      .then((data) => {
        setBackendOnline(true);
        setAllowSignup(Boolean(data?.allow_signup));
      })
      .catch(() => setBackendOnline(false));
  }, []);

  const handleLogin = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    setError(null);
    setSuccessMsg(null);

    const userToLogin = loginUsername.trim();
    if (!userToLogin || !loginPassword) {
      setError('Please provide your username/email and password.');
      return;
    }

    try {
      setLoading(true);
      const res = await login({
        username: userToLogin,
        password: loginPassword,
      });
      onAuthSuccess(res.user);
    } catch (err: unknown) {
      setError(getErrorMessage(err) || 'Login failed. Please check your credentials.');
    } finally {
      setLoading(false);
    }
  };


  const handleSignup = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccessMsg(null);

    if (!allowSignup) {
      setError('Public registration is disabled on this server.');
      return;
    }

    const username = signupUsername.trim();
    const email = signupEmail.trim();

    if (username.length < 3) {
      setError('Username must be at least 3 characters.');
      return;
    }
    if (!email.includes('@') || !email.includes('.')) {
      setError('Please provide a valid email address.');
      return;
    }
    if (signupPassword.length < 6) {
      setError('Password must be at least 6 characters.');
      return;
    }
    if (signupPassword !== signupConfirm) {
      setError('Passwords do not match.');
      return;
    }

    try {
      setLoading(true);
      const res = await signup({
        username,
        email,
        password: signupPassword,
      });
      onAuthSuccess(res.user);
    } catch (err: unknown) {
      setError(getErrorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100 flex flex-col justify-between relative">
      {/* Ambient background glow for desktop polish */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden">
        <div className="absolute -top-40 -left-40 w-96 h-96 bg-emerald-600/10 rounded-full blur-3xl" />
        <div className="absolute top-1/3 -right-40 w-96 h-96 bg-teal-600/10 rounded-full blur-3xl" />
        <div className="absolute -bottom-40 left-1/3 w-96 h-96 bg-emerald-900/15 rounded-full blur-3xl" />
      </div>

      {/* Top Header */}
      <header className="relative z-20 bg-slate-900/80 backdrop-blur-md border-b border-slate-800/80 py-3.5 shadow-sm">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-emerald-600 to-teal-500 flex items-center justify-center shadow-lg shadow-emerald-950/40 border border-emerald-400/20">
              <Coins className="w-5 h-5 text-white" />
            </div>
            <div className="flex items-center space-x-2">
              <span className="font-extrabold text-lg sm:text-xl tracking-tight text-white">
                GnuCash
              </span>
              <span className="text-[10px] uppercase font-bold tracking-wider px-2 py-0.5 rounded-full bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                Web
              </span>
            </div>
          </div>

          <div className="flex items-center space-x-3 text-xs">
            {backendOnline !== null && (
              <div className="hidden sm:inline-flex items-center space-x-1.5 px-2.5 py-1 rounded-full bg-slate-800/80 border border-slate-700/60 text-slate-300">
                <span
                  className={`w-2 h-2 rounded-full ${
                    backendOnline ? 'bg-emerald-400 animate-pulse' : 'bg-rose-500'
                  }`}
                />
                <span className="font-medium text-[11px]">
                  {backendOnline ? 'Rust Axum Engine Online' : 'Backend Connecting'}
                </span>
              </div>
            )}
            <span className="text-slate-400 hidden md:inline text-xs border-l border-slate-800 pl-3">
              Multi-Tenant Accounting
            </span>
          </div>
        </div>
      </header>

      {/* Main Content: Responsive Desktop 2-Column Showcase vs Mobile Centered */}
      <main className="relative z-10 flex-1 flex items-center justify-center py-8 sm:py-12 lg:py-16 px-4 sm:px-6 lg:px-8">
        <div className="max-w-7xl w-full mx-auto">
          {/* Mobile Brand Banner (visible only on small screens) */}
          <div className="text-center mb-6 lg:hidden">
            <div className="inline-flex items-center justify-center w-12 h-12 rounded-2xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 mb-2 shadow-lg shadow-emerald-900/30">
              <Landmark className="w-6 h-6" />
            </div>
            <h1 className="text-2xl font-extrabold text-white tracking-tight">
              GnuCash Web
            </h1>
            <p className="text-xs text-slate-400 mt-0.5">
              Pure Double-Entry Ledger & Financial Engine
            </p>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 lg:gap-12 xl:gap-16 items-center">
            {/* Desktop Left Column: Product Showcase & Financial Capabilities */}
            <div className="hidden lg:flex lg:col-span-7 flex-col space-y-8 pr-2">
              <div className="space-y-4">
                <div className="inline-flex items-center space-x-2 px-3 py-1 rounded-full bg-emerald-500/10 border border-emerald-500/25 text-emerald-400 text-xs font-semibold uppercase tracking-wider">
                  <Sparkles className="w-3.5 h-3.5 text-emerald-400" />
                  <span>Pure Double-Entry Financial Engine</span>
                </div>

                <h2 className="text-3xl xl:text-4xl 2xl:text-5xl font-black text-white tracking-tight leading-[1.15]">
                  Precision accounting & financial clarity,{' '}
                  <span className="text-transparent bg-clip-text bg-gradient-to-r from-emerald-400 via-teal-300 to-emerald-200">
                    built for the web
                  </span>
                  .
                </h2>

                <p className="text-sm xl:text-base text-slate-300 leading-relaxed max-w-xl">
                  GnuCash Web brings the mathematical precision of desktop double-entry accounting
                  to a modern browser experience. Manage hierarchical accounts, record multi-split
                  transactions, and generate real-time financial statements with zero-sum ledger integrity.
                </p>
              </div>

              {/* 2x2 Feature Grid */}
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div className="p-4 rounded-xl bg-slate-900/70 border border-slate-800/80 backdrop-blur-xs hover:border-emerald-500/40 transition">
                  <div className="w-8 h-8 rounded-lg bg-emerald-500/10 text-emerald-400 flex items-center justify-center mb-2.5 border border-emerald-500/20">
                    <Scale className="w-4 h-4" />
                  </div>
                  <h3 className="text-sm font-bold text-white mb-1">
                    Strict Double-Entry Ledger
                  </h3>
                  <p className="text-xs text-slate-400 leading-normal">
                    Debits strictly equal credits on every transaction. Guaranteed mathematical integrity with PostgreSQL transaction atomicity.
                  </p>
                </div>

                <div className="p-4 rounded-xl bg-slate-900/70 border border-slate-800/80 backdrop-blur-xs hover:border-emerald-500/40 transition">
                  <div className="w-8 h-8 rounded-lg bg-teal-500/10 text-teal-400 flex items-center justify-center mb-2.5 border border-teal-500/20">
                    <TrendingUp className="w-4 h-4" />
                  </div>
                  <h3 className="text-sm font-bold text-white mb-1">
                    Real-Time Financial Reports
                  </h3>
                  <p className="text-xs text-slate-400 leading-normal">
                    Instant Balance Sheets, Profit & Loss (Income Statement), and Trial Balances calculated dynamically from your journal.
                  </p>
                </div>

                <div className="p-4 rounded-xl bg-slate-900/70 border border-slate-800/80 backdrop-blur-xs hover:border-emerald-500/40 transition">
                  <div className="w-8 h-8 rounded-lg bg-emerald-500/10 text-emerald-400 flex items-center justify-center mb-2.5 border border-emerald-500/20">
                    <Layers className="w-4 h-4" />
                  </div>
                  <h3 className="text-sm font-bold text-white mb-1">
                    Chart of Accounts & Registers
                  </h3>
                  <p className="text-xs text-slate-400 leading-normal">
                    Unlimited hierarchy across 5 core types (Assets, Liabilities, Equity, Income, Expenses) with checkbook register views.
                  </p>
                </div>

                <div className="p-4 rounded-xl bg-slate-900/70 border border-slate-800/80 backdrop-blur-xs hover:border-emerald-500/40 transition">
                  <div className="w-8 h-8 rounded-lg bg-teal-500/10 text-teal-400 flex items-center justify-center mb-2.5 border border-teal-500/20">
                    <ShieldCheck className="w-4 h-4" />
                  </div>
                  <h3 className="text-sm font-bold text-white mb-1">
                    Isolated Multi-User Security
                  </h3>
                  <p className="text-xs text-slate-400 leading-normal">
                    Strict per-user data segregation, Argon2 password encryption, and native import for GnuCash SQLite & XML files.
                  </p>
                </div>
              </div>

              {/* Specs & Highlights Pills */}
              <div className="flex flex-wrap items-center gap-3 pt-2 text-xs text-slate-400">
                <span className="inline-flex items-center space-x-1.5 text-slate-300">
                  <Check className="w-3.5 h-3.5 text-emerald-400" />
                  <span>Zero-Sum Balance Enforced</span>
                </span>
                <span className="text-slate-700">•</span>
                <span className="inline-flex items-center space-x-1.5 text-slate-300">
                  <Check className="w-3.5 h-3.5 text-emerald-400" />
                  <span>Bank Statement Reconcile</span>
                </span>
                <span className="text-slate-700">•</span>
                <span className="inline-flex items-center space-x-1.5 text-slate-300">
                  <Check className="w-3.5 h-3.5 text-emerald-400" />
                  <span>Multi-Currency Ready (INR, USD, EUR, GBP)</span>
                </span>
              </div>
            </div>

            {/* Right Column: Authentication Card */}
            <div className="lg:col-span-5 w-full max-w-md mx-auto lg:max-w-none">
              <div className="bg-white text-slate-900 rounded-2xl shadow-2xl shadow-black/60 border border-slate-200 overflow-hidden">
                {/* Tab Selector (Sign In vs Create Account) */}
                {tab !== 'reset' ? (
                  <div className="flex border-b border-slate-200 bg-slate-50/80">
                    <button
                      type="button"
                      onClick={() => {
                        setTab('login');
                        setError(null);
                        setSuccessMsg(null);
                      }}
                      className={`flex-1 py-3.5 text-xs sm:text-sm font-bold transition text-center relative ${
                        tab === 'login'
                          ? 'bg-white text-emerald-700 shadow-xs'
                          : 'text-slate-500 hover:text-slate-800 hover:bg-slate-100/60'
                      }`}
                    >
                      <span>Sign In</span>
                      {tab === 'login' && (
                        <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-emerald-600" />
                      )}
                    </button>
                    {allowSignup && (
                      <button
                        type="button"
                        onClick={() => {
                          setTab('signup');
                          setError(null);
                          setSuccessMsg(null);
                        }}
                        className={`flex-1 py-3.5 text-xs sm:text-sm font-bold transition text-center relative ${
                          tab === 'signup'
                            ? 'bg-white text-emerald-700 shadow-xs'
                            : 'text-slate-500 hover:text-slate-800 hover:bg-slate-100/60'
                        }`}
                      >
                        <span>Create Account</span>
                        {tab === 'signup' && (
                          <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-emerald-600" />
                        )}
                      </button>
                    )}
                  </div>
                ) : (
                  <div className="flex items-center justify-between border-b border-slate-200 bg-slate-50 px-5 py-3">
                    <span className="text-xs sm:text-sm font-bold text-slate-800">
                      Password Recovery
                    </span>
                    <button
                      type="button"
                      onClick={() => {
                        setTab('login');
                        setError(null);
                      }}
                      className="text-xs text-emerald-600 hover:text-emerald-700 font-semibold"
                    >
                      Back to Sign In
                    </button>
                  </div>
                )}

                <div className="p-6 sm:p-8">
                  {/* Card Title & Subtitle */}
                  {tab === 'login' && (
                    <div className="mb-5">
                      <h3 className="text-lg sm:text-xl font-bold text-slate-900 tracking-tight">
                        Welcome back
                      </h3>
                      <p className="text-xs sm:text-sm text-slate-500 mt-1">
                        Enter your credentials to access your financial ledger
                      </p>
                    </div>
                  )}

                  {tab === 'signup' && (
                    <div className="mb-5">
                      <h3 className="text-lg sm:text-xl font-bold text-slate-900 tracking-tight">
                        Create your ledger
                      </h3>
                      <p className="text-xs sm:text-sm text-slate-500 mt-1">
                        Sign up to start tracking accounts with double-entry precision
                      </p>
                    </div>
                  )}

                  {/* Alerts */}
                  {error && (
                    <div className="mb-5 p-3.5 bg-rose-50 border border-rose-200 text-rose-800 rounded-xl text-xs flex items-start space-x-2.5">
                      <AlertCircle className="w-4 h-4 shrink-0 text-rose-500 mt-0.5" />
                      <span className="font-medium leading-relaxed">{error}</span>
                    </div>
                  )}

                  {successMsg && (
                    <div className="mb-5 p-3.5 bg-emerald-50 border border-emerald-200 text-emerald-800 rounded-xl text-xs flex items-start space-x-2.5">
                      <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-600 mt-0.5" />
                      <span className="font-medium leading-relaxed">{successMsg}</span>
                    </div>
                  )}

                  {/* Login Form */}
                  {tab === 'login' && (
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
                  )}

                  {/* Signup Form */}
                  {tab === 'signup' && (
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
                  ))}

                  {/* Reset Password Guidance */}
                  {tab === 'reset' && (
                    <div className="space-y-4">
                      <div className="p-3.5 bg-slate-900 border border-slate-800 rounded-xl text-xs text-slate-300 space-y-2">
                        <p className="font-semibold text-amber-400 flex items-center space-x-1.5">
                          <KeyRound className="w-4 h-4 text-amber-400" />
                          <span>CLI Admin Password Reset</span>
                        </p>
                        <p className="text-[11px] text-slate-400 leading-relaxed">
                          To protect against account lockout, username enumeration, and unauthorized password resets over the network, self-service password reset is disabled on this server.
                        </p>
                        <div className="bg-slate-950 p-2.5 rounded-lg border border-slate-800 font-mono text-[11px] text-emerald-400 space-y-1">
                          <p className="text-slate-400 text-[10px]"># Run in repository on server console / SSH:</p>
                          <p>./scripts/reset_password.sh &lt;username&gt; [new_password]</p>
                        </div>
                        <p className="text-[11px] text-slate-400">
                          If <code className="text-emerald-400 font-mono text-[10px]">new_password</code> is omitted, a random temporary password will be written to <code className="text-amber-300 font-mono text-[10px]">password_reset.txt</code> on the server.
                        </p>
                      </div>

                      <div className="pt-2">
                        <button
                          type="button"
                          onClick={() => {
                            setTab('login');
                            setError(null);
                          }}
                          className="w-full py-2.5 px-4 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs sm:text-sm font-semibold transition shadow-sm text-center flex items-center justify-center space-x-1.5"
                        >
                          <ArrowRight className="w-4 h-4" />
                          <span>Back to Sign In</span>
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      </main>

      {/* Footer */}
      <footer className="relative z-20 border-t border-slate-800/80 bg-slate-900/60 backdrop-blur-md py-4 text-xs text-slate-400">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 flex flex-col sm:flex-row items-center justify-between gap-2 text-center sm:text-left">
          <p className="text-slate-400">
            GnuCash Web • Pure Double-Entry Accounting Engine
          </p>
          <p className="text-slate-500">
            Isolated Tenant Ledgers • Multi-Currency Support • Port 8081
          </p>
        </div>
      </footer>
    </div>
  );
};
