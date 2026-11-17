import React from 'react';
import { ArrowRight, KeyRound } from 'lucide-react';
import { AuthTab, Setter } from './types';

interface ResetPasswordGuidanceProps {
  setTab: Setter<AuthTab>;
  setError: Setter<string | null>;
}

/** Explains that passwords are reset by the server administrator via the CLI. */
export const ResetPasswordGuidance: React.FC<ResetPasswordGuidanceProps> = ({
  setTab,
  setError,
}) => (
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

);
