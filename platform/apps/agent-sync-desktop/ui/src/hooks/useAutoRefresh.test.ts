import { renderHook } from "@testing-library/react";
import { act } from "react";
import { describe, expect, it, vi } from "vitest";
import { useAutoRefresh } from "./useAutoRefresh";

describe("useAutoRefresh", () => {
  it("schedules refresh ticks and exposes next run time", () => {
    vi.useFakeTimers();
    const onRefresh = vi.fn();

    const { result } = renderHook(() =>
      useAutoRefresh({
        enabled: true,
        intervalMinutes: 5,
        onRefresh,
        resetSignal: 0,
      }),
    );

    expect(result.current.nextRunAt).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });

    expect(onRefresh).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });

  it("disables timer when interval is manual", () => {
    vi.useFakeTimers();
    const onRefresh = vi.fn();

    const { result } = renderHook(() =>
      useAutoRefresh({
        enabled: true,
        intervalMinutes: 0,
        onRefresh,
        resetSignal: 0,
      }),
    );

    expect(result.current.nextRunAt).toBeNull();

    act(() => {
      vi.advanceTimersByTime(30 * 60 * 1000);
    });

    expect(onRefresh).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("clears next run when auto refresh is disabled", () => {
    vi.useFakeTimers();
    const onRefresh = vi.fn();

    const { result, rerender } = renderHook(
      ({
        enabled,
        intervalMinutes,
      }: {
        enabled: boolean;
        intervalMinutes: 0 | 5 | 15 | 30;
      }) =>
        useAutoRefresh({
          enabled,
          intervalMinutes,
          onRefresh,
          resetSignal: 0,
        }),
      {
        initialProps: {
          enabled: true,
          intervalMinutes: 5,
        },
      },
    );

    expect(result.current.nextRunAt).not.toBeNull();

    rerender({
      enabled: false,
      intervalMinutes: 5,
    });
    act(() => {
      vi.advanceTimersByTime(0);
    });

    expect(result.current.nextRunAt).toBeNull();
    vi.useRealTimers();
  });

  it("attaches rejection handler for async refresh results", () => {
    vi.useFakeTimers();
    const refreshPromise = Promise.resolve();
    const catchSpy = vi.spyOn(refreshPromise, "catch");
    const onRefresh = vi.fn().mockReturnValue(refreshPromise);

    renderHook(() =>
      useAutoRefresh({
        enabled: true,
        intervalMinutes: 5,
        onRefresh,
        resetSignal: 0,
      }),
    );

    act(() => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });

    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(catchSpy).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
  });
});
