import { useEffect, useRef, useState } from "react";
import { errorMessage } from "../lib/utils";

export function useEntityDetails<T>(
  selectedKey: string | null,
  fetcher: (key: string) => Promise<T>,
  onError: (message: string) => void,
): T | null {
  const [details, setDetails] = useState<T | null>(null);
  const requestRef = useRef(0);

  useEffect(() => {
    if (!selectedKey) {
      requestRef.current += 1;
      queueMicrotask(() => setDetails(null));
      return;
    }

    const requestId = ++requestRef.current;

    void (async () => {
      try {
        const next = await fetcher(selectedKey);
        if (requestId !== requestRef.current) {
          return;
        }
        setDetails(next);
      } catch (error) {
        if (requestId !== requestRef.current) {
          return;
        }
        onError(errorMessage(error));
      }
    })();
  }, [onError, selectedKey, fetcher]);

  return details;
}
