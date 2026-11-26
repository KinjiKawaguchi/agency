import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClientProvider, useSuspenseQuery } from '@tanstack/react-query'
import { I18nProvider } from '../i18n'
import { ApiError } from '../api/client'
import { createQueryClient } from '../app/queryClient'
import { Loading, Skeleton } from './Skeleton'
import { Region } from './Region'

/** A read that fails with `failures` 503s and then answers `text`. */
function reader(failures: number, text: string) {
  let calls = 0
  return {
    calls: () => calls,
    Content() {
      const { data } = useSuspenseQuery({
        queryKey: ['region-test'],
        queryFn: () => (calls++ < failures ? Promise.reject(new ApiError('The board has not been read yet.', 503)) : Promise.resolve(text)),
        retry: false,
      })
      return <p>{data}</p>
    },
  }
}

function renderRegion(Content: () => React.JSX.Element) {
  return render(
    <I18nProvider initial="en">
      <QueryClientProvider client={createQueryClient([0])}>
        <Region what="the thing" skeleton={<Loading><Skeleton /></Loading>}>
          <Content />
        </Region>
      </QueryClientProvider>
    </I18nProvider>,
  )
}

describe('a region', () => {
  it('shows its skeleton while it reads, then its content', async () => {
    const { Content } = reader(0, 'Ready')
    renderRegion(Content)
    expect(screen.getByRole('status')).toHaveTextContent('Loading…')
    expect(await screen.findByText('Ready')).toBeInTheDocument()
    expect(screen.queryByRole('status')).toBeNull()
  })

  it('says what failed, why, and when it tries again, and tries again when asked', async () => {
    const user = userEvent.setup()
    const { Content, calls } = reader(1, 'Back')
    renderRegion(Content)
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Could not load the thing: The board has not been read yet.')
    expect(alert).toHaveTextContent('Trying again in 30 s.')
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByText('Back')).toBeInTheDocument()
    expect(calls()).toBe(2)
  })

  it('counts down and tries again by itself', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    try {
      const { Content } = reader(1, 'Back by itself')
      renderRegion(Content)
      expect(await screen.findByRole('alert')).toHaveTextContent('Trying again in 30 s.')
      await act(() => vi.advanceTimersByTimeAsync(10_000))
      expect(screen.getByRole('alert')).toHaveTextContent('Trying again in 20 s.')
      await act(() => vi.advanceTimersByTimeAsync(20_000))
      expect(await screen.findByText('Back by itself')).toBeInTheDocument()
    } finally {
      vi.useRealTimers()
    }
  })
})
