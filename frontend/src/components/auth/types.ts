import React from 'react';

export type AuthTab = 'login' | 'signup' | 'reset';

export type Setter<T> = React.Dispatch<React.SetStateAction<T>>;
