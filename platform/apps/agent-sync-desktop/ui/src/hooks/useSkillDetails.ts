import { useEffect, useRef, useState } from "react";
import { getSkillDetails } from "../tauriApi";
import type { SkillDetails } from "../types";

type UseSkillDetailsOptions = {
  selectedSkillKey: string | null;
  onError: (message: string) => void;
};

type UseSkillDetailsResult = {
  details: SkillDetails | null;
  renameDraft: string;
  setRenameDraft: (value: string | ((prev: string) => string)) => void;
};

export function useSkillDetails({
  selectedSkillKey,
  onError,
}: UseSkillDetailsOptions): UseSkillDetailsResult {
  const [details, setDetails] = useState<SkillDetails | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const requestRef = useRef(0);

  useEffect(() => {
    if (!selectedSkillKey) {
      requestRef.current += 1;
      const resetTimer = window.setTimeout(() => {
        setDetails(null);
        setRenameDraft("");
      }, 0);
      return () => {
        window.clearTimeout(resetTimer);
      };
    }

    const requestId = ++requestRef.current;

    void (async () => {
      try {
        const next = await getSkillDetails(selectedSkillKey);
        if (requestId !== requestRef.current) {
          return;
        }
        setDetails(next);
        setRenameDraft(next.skill.name);
      } catch (error) {
        if (requestId !== requestRef.current) {
          return;
        }
        onError(String(error));
      }
    })();
  }, [onError, selectedSkillKey]);

  return {
    details,
    renameDraft,
    setRenameDraft,
  };
}
