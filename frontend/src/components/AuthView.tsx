import React, { useState } from 'react';
import {
  Landmark,
  Lock,
  User as UserIcon,
  Mail,
  AlertCircle,
  ArrowRight,
  ShieldCheck,
  CheckCircle2,
  Eye,
  EyeOff,
  RotateCcw,
  KeyRound,
} from 'lucide-react';
import { login, signup, resetPassword } from '../api';
import { User } from '../types';

interface AuthViewProps {
  onAuthSuccess: (user: User) => void;
}

export const AuthView: React.FC<AuthViewProps> = ({ onAuthSuccess }) => {
  const [tab, setTab] = useState<'login' | 'signup' | 'reset'>('login');

  // Login form state - pre-filled with default user for immediate frictionless access
  const [loginUsername, setLoginUsername] = useState('kishan');
  const [loginPassword, setLoginPassword] = useState('password123');
  const [showPassword, setShowPassword] = useState(false);

  // Signup form state
  const [signupUsername, setSignupUsername] = useState('');
  const [signupEmail, setSignupEmail] = useState('');
  const [signupPassword, setSignupPassword] = useState('');
  const [signupConfirm, setSignupConfirm] = useState('');

  // Reset form state
  const [resetUsername, setResetUsername] = useState('kishan');
  const [resetNewPassword, setResetNewPassword] = useState('password123');

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

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
    } catch (err: any) {
      setError(
        err.message ||
          'Login failed. If using the default account, make sure username is "kishan" and password is "password123".'
      );
    } finally {
      setLoading(false);
    }
  };

  const handleQuickDemoLogin = async () => {
    setError(null);
    setSuccessMsg(null);
    setLoginUsername('kishan');
    setLoginPassword('password123');
    try {
      setLoading(true);
      const res = await login({
        username: 'kishan',
        password: 'password123',
      });
      onAuthSuccess(res.user);
    } catch (err: any) {
      setError(
        err.message ||
          'Demo login failed. You can reset the password for "kishan" to "password123" below.'
      );
    } finally {
      setLoading(false);
    }
  };

  const handleResetPassword = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccessMsg(null);

    const targetUser = resetUsername.trim();
    if (!targetUser) {
      setError('Please provide the username to reset.');
      return;
    }
    if (resetNewPassword.length < 6) {
      setError('New password must be at least 6 characters.');
      return;
    }

    try {
      setLoading(true);
      const res = await resetPassword({
        username: targetUser,
        new_password: resetNewPassword,
      });
      setSuccessMsg(res.message || 'Password successfully reset! You can now log in.');
      setLoginUsername(targetUser);
      setLoginPassword(resetNewPassword);
      setTab('login');
    } catch (err: any) {
      setError(err.message || 'Failed to reset password.');
    } finally {
      setLoading(false);
    }
  };

  const handleSignup = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccessMsg(null);

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
    } catch (err: any) {
      setError(err.message || 'Signup failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-slate-900 via-slate-800 to-emerald-950 flex flex-col items-center justify-center p-4">
      {/* Container */}
      <div className="max-w-md w-full">
        {/* Brand Header */}
        <div className="text-center mb-6 sm:mb-8">
          <div className="inline-flex items-center justify-center w-14 h-14 rounded-2xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 mb-3 shadow-lg shadow-emerald-900/30">
            <Landmark className="w-8 h-8" />
          </div>
          <h1 className="text-2xl sm:text-3xl font-extrabold text-white tracking-tight">
            GnuCash Web
          </h1>
          <p className="text-xs sm:text-sm text-slate-400 mt-1">
            Pure Double-Entry Ledger & Financial Engine
          </p>
        </div>

        {/* Card */}
        <div className="bg-white rounded-2xl shadow-2xl border border-slate-200 overflow-hidden">
          {/* Tab Selector */}
          <div className="flex border-b border-slate-200 bg-slate-50">
            <button
              type="button"
              onClick={() => {
                setTab('login');
                setError(null);
                setSuccessMsg(null);
              }}
              className={`flex-1 py-3 text-xs sm:text-sm font-semibold transition text-center ${
                tab === 'login'
                  ? 'bg-white text-emerald-700 border-b-2 border-emerald-600 shadow-sm'
                  : 'text-slate-500 hover:text-slate-800 hover:bg-slate-100/60'
              }`}
            >
              Sign In
            </button>
            <button
              type="button"
              onClick={() => {
                setTab('signup');
                setError(null);
                setSuccessMsg(null);
              }}
              className={`flex-1 py-3 text-xs sm:text-sm font-semibold transition text-center ${
                tab === 'signup'
                  ? 'bg-white text-emerald-700 border-b-2 border-emerald-600 shadow-sm'
                  : 'text-slate-500 hover:text-slate-800 hover:bg-slate-100/60'
              }`}
            >
              Create Account
            </button>
            <button
              type="button"
              onClick={() => {
                setTab('reset');
                setError(null);
                setSuccessMsg(null);
              }}
              className={`py-3 px-3 text-xs font-semibold transition text-center ${
                tab === 'reset'
                  ? 'bg-white text-emerald-700 border-b-2 border-emerald-600 shadow-sm'
                  : 'text-slate-500 hover:text-slate-800 hover:bg-slate-100/60'
              }`}
              title="Reset Password"
            >
              Reset
            </button>
          </div>

          <div className="p-5 sm:p-7">
            {error && (
              <div className="mb-4 p-3 bg-rose-50 border border-rose-200 text-rose-700 rounded-xl text-xs space-y-2">
                <div className="flex items-center space-x-2">
                  <AlertCircle className="w-4 h-4 shrink-0 text-rose-500" />
                  <span className="font-medium">{error}</span>
                </div>
                {tab === 'login' && (
                  <button
                    type="button"
                    onClick={() => {
                      setResetUsername('kishan');
                      setResetNewPassword('password123');
                      setTab('reset');
                      setError(null);
                    }}
                    className="text-[11px] text-rose-800 font-semibold underline hover:text-rose-900 block"
                  >
                    Click here to reset "kishan" password to "password123"
                  </button>
                )}
              </div>
            )}

            {successMsg && (
              <div className="mb-4 p-3 bg-emerald-50 border border-emerald-200 text-emerald-800 rounded-xl text-xs flex items-center space-x-2">
                <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-600" />
                <span className="font-medium">{successMsg}</span>
              </div>
            )}

            {/* Login Tab */}
            {tab === 'login' && (
              <div className="space-y-4">
                {/* Default Credentials Banner */}
                <div className="p-3 bg-emerald-50/80 border border-emerald-200 rounded-xl flex items-center justify-between gap-2">
                  <div className="flex items-center space-x-2 text-xs text-emerald-900">
                    <ShieldCheck className="w-4 h-4 text-emerald-600 shrink-0" />
                    <div>
                      <p className="font-semibold text-emerald-950">Default Account</p>
                      <p className="text-[11px] text-emerald-700 font-mono">
                        User: <strong>kishan</strong> • Pass: <strong>password123</strong>
                      </p>
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={handleQuickDemoLogin}
                    disabled={loading}
                    className="px-2.5 py-1 text-xs font-semibold bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg transition shadow-xs shrink-0"
                  >
                    1-Click Login
                  </button>
                </div>

                <form onSubmit={handleLogin} className="space-y-4">
                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      Username or Email
                    </label>
                    <div className="relative">
                      <UserIcon className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
                      <input
                        type="text"
                        required
                        placeholder="Username or email address"
                        value={loginUsername}
                        onChange={(e) => setLoginUsername(e.target.value)}
                        className="w-full pl-9 pr-3 py-2 text-xs sm:text-sm bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
                      />
                    </div>
                  </div>

                  <div>
                    <div className="flex items-center justify-between mb-1">
                      <label className="block text-xs font-semibold text-slate-700">
                        Password
                      </label>
                      <button
                        type="button"
                        onClick={() => {
                          setTab('reset');
                          setError(null);
                        }}
                        className="text-[11px] text-emerald-600 hover:underline font-medium"
                      >
                        Forgot password?
                      </button>
                    </div>
                    <div className="relative">
                      <Lock className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
                      <input
                        type={showPassword ? 'text' : 'password'}
                        required
                        placeholder="••••••••"
                        value={loginPassword}
                        onChange={(e) => setLoginPassword(e.target.value)}
                        className="w-full pl-9 pr-10 py-2 text-xs sm:text-sm bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-mono"
                      />
                      <button
                        type="button"
                        onClick={() => setShowPassword(!showPassword)}
                        className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-600"
                      >
                        {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                      </button>
                    </div>
                  </div>

                  <button
                    type="submit"
                    disabled={loading}
                    className="w-full py-2.5 px-4 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs sm:text-sm font-semibold transition shadow-sm disabled:opacity-50 flex items-center justify-center space-x-1.5 active:scale-[0.99]"
                  >
                    <span>{loading ? 'Signing in...' : 'Sign In'}</span>
                    {!loading && <ArrowRight className="w-4 h-4" />}
                  </button>
                </form>
              </div>
            )}

            {/* Signup Tab */}
            {tab === 'signup' && (
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
                      placeholder="e.g. kishan, accountant_jane"
                      value={signupUsername}
                      onChange={(e) => setSignupUsername(e.target.value)}
                      className="w-full pl-9 pr-3 py-2 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
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
                      placeholder="e.g. jane@example.com"
                      value={signupEmail}
                      onChange={(e) => setSignupEmail(e.target.value)}
                      className="w-full pl-9 pr-3 py-2 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
                    />
                  </div>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      Password *
                    </label>
                    <div className="relative">
                      <Lock className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
                      <input
                        type="password"
                        required
                        placeholder="At least 6 chars"
                        value={signupPassword}
                        onChange={(e) => setSignupPassword(e.target.value)}
                        className="w-full pl-9 pr-3 py-2 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-mono"
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-xs font-semibold text-slate-700 mb-1">
                      Confirm Password *
                    </label>
                    <div className="relative">
                      <Lock className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
                      <input
                        type="password"
                        required
                        placeholder="Repeat password"
                        value={signupConfirm}
                        onChange={(e) => setSignupConfirm(e.target.value)}
                        className="w-full pl-9 pr-3 py-2 text-xs bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-mono"
                      />
                    </div>
                  </div>
                </div>

                <div className="p-2.5 bg-emerald-50/70 border border-emerald-100 rounded-lg flex items-start space-x-2 text-[11px] text-emerald-800">
                  <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0 mt-0.5" />
                  <span>
                    Your personal ledger will be automatically initialized with standard root accounts (Assets, Liabilities, Equity, Income, Expenses).
                  </span>
                </div>

                <button
                  type="submit"
                  disabled={loading}
                  className="w-full py-2.5 px-4 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs sm:text-sm font-semibold transition shadow-sm disabled:opacity-50 flex items-center justify-center space-x-1.5 active:scale-[0.99] mt-2"
                >
                  <span>{loading ? 'Creating Account...' : 'Create Account & Sign In'}</span>
                  {!loading && <ArrowRight className="w-4 h-4" />}
                </button>
              </form>
            )}

            {/* Reset Password Tab */}
            {tab === 'reset' && (
              <form onSubmit={handleResetPassword} className="space-y-4">
                <div className="p-3 bg-amber-50 border border-amber-200 rounded-xl text-xs text-amber-900 space-y-1">
                  <p className="font-semibold flex items-center space-x-1.5">
                    <KeyRound className="w-3.5 h-3.5 text-amber-700" />
                    <span>Reset Account Password</span>
                  </p>
                  <p className="text-[11px] text-amber-700">
                    Enter the username and the new password you wish to set.
                  </p>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    Username or Email
                  </label>
                  <div className="relative">
                    <UserIcon className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
                    <input
                      type="text"
                      required
                      placeholder="e.g. kishan"
                      value={resetUsername}
                      onChange={(e) => setResetUsername(e.target.value)}
                      className="w-full pl-9 pr-3 py-2 text-xs sm:text-sm bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500"
                    />
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">
                    New Password (at least 6 characters)
                  </label>
                  <div className="relative">
                    <Lock className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
                    <input
                      type="password"
                      required
                      placeholder="e.g. password123"
                      value={resetNewPassword}
                      onChange={(e) => setResetNewPassword(e.target.value)}
                      className="w-full pl-9 pr-3 py-2 text-xs sm:text-sm bg-slate-50 border border-slate-300 rounded-lg focus:ring-2 focus:ring-emerald-500/20 focus:border-emerald-500 font-mono"
                    />
                  </div>
                </div>

                <div className="flex items-center space-x-2 pt-1">
                  <button
                    type="submit"
                    disabled={loading}
                    className="flex-1 py-2 px-3 bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg text-xs font-semibold transition shadow-xs flex items-center justify-center space-x-1.5"
                  >
                    <RotateCcw className="w-3.5 h-3.5" />
                    <span>{loading ? 'Resetting...' : 'Set New Password'}</span>
                  </button>

                  <button
                    type="button"
                    onClick={() => {
                      setTab('login');
                      setError(null);
                    }}
                    className="py-2 px-3 bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-lg text-xs font-medium transition"
                  >
                    Cancel
                  </button>
                </div>
              </form>
            )}
          </div>
        </div>

        {/* Footer */}
        <p className="text-center text-xs text-slate-500 mt-6">
          Multi-User Double-Entry Accounting • Data isolated per user
        </p>
      </div>
    </div>
  );
};
