import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { useAppUpdate } from './use-app-update'
const api = vi.hoisted(() => ({ check: vi.fn() }))
vi.mock('../lib/desktop-api', () => ({ checkAppUpdate: api.check }))
afterEach(() => { cleanup(); api.check.mockReset() })
it('changing channel drops an in-flight preview result and accepts the new stable check', async () => {
  let old!: (version: string) => void
  api.check.mockReturnValueOnce(new Promise(resolve => { old = resolve })).mockResolvedValueOnce(null)
  const { result } = renderHook(() => useAppUpdate(false))
  await waitFor(() => expect(api.check).toHaveBeenCalledTimes(1))
  await act(() => result.current.check(true))
  await act(async () => { old('1.8.0-preview.1') })
  expect(result.current.version).toBeNull()
  expect(api.check).toHaveBeenCalledTimes(2)
})
it('rechecks share one request and update installation cannot start a second check', async () => {
  let finish!: (version: string) => void
  api.check.mockReturnValue(new Promise(resolve => { finish = resolve }))
  const { result, rerender } = renderHook(({ installing }) => useAppUpdate(installing), { initialProps: { installing: false } })
  const first = result.current.check(); expect(result.current.check()).toBe(first)
  rerender({ installing: true }); await result.current.check(true)
  expect(api.check).toHaveBeenCalledTimes(1)
  await act(async () => { finish('2.0.0') })
  expect(result.current.version).toBeNull()
})
