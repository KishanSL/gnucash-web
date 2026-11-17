import React from 'react';
import { Scale, TrendingUp, Layers, ShieldCheck, Sparkles, Check } from 'lucide-react';

/** Desktop-only product showcase beside the sign-in card. */
export const AuthShowcase: React.FC = () => (
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
);
