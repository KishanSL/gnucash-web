import React, { useState, useEffect } from 'react';
import {
  Landmark,
  AlertCircle,
  CheckCircle2,
  Coins,
} from 'lucide-react';
import { login, signup, checkHealth, getErrorMessage } from '../api';
import { User } from '../types';
import { AuthShowcase } from './auth/AuthShowcase';
import { LoginForm } from './auth/LoginForm';
import { SignupForm } from './auth/SignupForm';
import { ResetPasswordGuidance } from './auth/ResetPasswordGuidance';
import { AuthTab } from './auth/types';

interface AuthViewProps {
  onAuthSuccess: (user: User) => void;
}

export const AuthView: React.FC<AuthViewProps> = ({ onAuthSuccess }) => {
  const [tab, setTab] = useState<AuthTab>('login');

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
            <AuthShowcase />

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
                    <LoginForm
                      setTab={setTab}
                      loginUsername={loginUsername}
                      setLoginUsername={setLoginUsername}
                      loginPassword={loginPassword}
                      setLoginPassword={setLoginPassword}
                      showLoginPassword={showLoginPassword}
                      setShowLoginPassword={setShowLoginPassword}
                      loading={loading}
                      setError={setError}
                      allowSignup={allowSignup}
                      handleLogin={handleLogin}
                    />
                  )}

                  {/* Signup Form */}
                  {tab === 'signup' && (
                    <SignupForm
                      setTab={setTab}
                      signupUsername={signupUsername}
                      setSignupUsername={setSignupUsername}
                      signupEmail={signupEmail}
                      setSignupEmail={setSignupEmail}
                      signupPassword={signupPassword}
                      setSignupPassword={setSignupPassword}
                      signupConfirm={signupConfirm}
                      setSignupConfirm={setSignupConfirm}
                      showSignupPassword={showSignupPassword}
                      setShowSignupPassword={setShowSignupPassword}
                      loading={loading}
                      setError={setError}
                      allowSignup={allowSignup}
                      handleSignup={handleSignup}
                    />
                  )}

                  {/* Reset Password Guidance */}
                  {tab === 'reset' && (
                    <ResetPasswordGuidance
                      setTab={setTab}
                      setError={setError}
                    />
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
