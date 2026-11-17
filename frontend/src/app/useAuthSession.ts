import { useCallback, useEffect, useState } from 'react';
import { User } from '../types';
import { fetchCurrentUser, getStoredToken, getStoredUser, logout } from '../api';

interface AuthSessionOptions {
  clearData: () => void;
  loadInitialData: () => Promise<void>;
  setLoading: (loading: boolean) => void;
}

/** Current user, logout, 401 handling and token verification on mount. */
export function useAuthSession({ clearData, loadInitialData, setLoading }: AuthSessionOptions) {
  const [currentUser, setCurrentUser] = useState<User | null>(() => getStoredUser());

  const handleLogout = useCallback(() => {
    logout();
    setCurrentUser(null);
    clearData();
  }, [clearData]);

  // Listen for 401 unauthorized token invalidation
  useEffect(() => {
    const handleAuthInvalid = () => {
      handleLogout();
    };
    window.addEventListener('gnucash_auth_invalid', handleAuthInvalid);
    return () => {
      window.removeEventListener('gnucash_auth_invalid', handleAuthInvalid);
    };
  }, [handleLogout]);

  // Sync / verify current user on mount if token exists
  useEffect(() => {
    if (getStoredToken()) {
      fetchCurrentUser()
        .then((user) => {
          setCurrentUser(user);
          loadInitialData();
        })
        .catch(() => {
          handleLogout();
        });
    } else {
      setLoading(false);
    }
  }, [handleLogout, loadInitialData, setLoading]);

  const handleAuthSuccess = (user: User) => {
    setCurrentUser(user);
    loadInitialData();
  };

  return { currentUser, handleLogout, handleAuthSuccess };
}
