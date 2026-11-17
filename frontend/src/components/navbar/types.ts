import React from 'react';

export type ActiveTab = 'accounts' | 'register' | 'journal' | 'reconcile' | 'reports';

export interface NavItem {
  id: ActiveTab;
  label: string;
  icon: React.ReactNode;
}

export type Setter<T> = React.Dispatch<React.SetStateAction<T>>;
