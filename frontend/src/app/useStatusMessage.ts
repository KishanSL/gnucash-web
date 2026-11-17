import { useCallback, useState } from 'react';

/** Status banner text, optionally cleared after a delay. */
export function useStatusMessage() {
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const flashStatus = useCallback((message: string, durationMs: number) => {
    setStatusMessage(message);
    setTimeout(() => setStatusMessage(null), durationMs);
  }, []);

  return { statusMessage, setStatusMessage, flashStatus };
}
