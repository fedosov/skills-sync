import { useEffect, useRef, useState } from "react";
import { getSubagentDetails } from "../tauriApi";
import type { SubagentDetails } from "../types";

type UseSubagentDetailsOptions = {
  selectedSubagentId: string | null;
  onError: (message: string) => void;
};

type UseSubagentDetailsResult = {
  subagentDetails: SubagentDetails | null;
};

export function useSubagentDetails({
  selectedSubagentId,
  onError,
}: UseSubagentDetailsOptions): UseSubagentDetailsResult {
  const [subagentDetails, setSubagentDetails] =
    useState<SubagentDetails | null>(null);
  const requestRef = useRef(0);

  useEffect(() => {
    if (!selectedSubagentId) {
      requestRef.current += 1;
      const resetTimer = window.setTimeout(() => {
        setSubagentDetails(null);
      }, 0);
      return () => {
        window.clearTimeout(resetTimer);
      };
    }

    const requestId = ++requestRef.current;

    void (async () => {
      try {
        const next = await getSubagentDetails(selectedSubagentId);
        if (requestId !== requestRef.current) {
          return;
        }
        setSubagentDetails(next);
      } catch (error) {
        if (requestId !== requestRef.current) {
          return;
        }
        onError(String(error));
      }
    })();
  }, [onError, selectedSubagentId]);

  return { subagentDetails };
}
