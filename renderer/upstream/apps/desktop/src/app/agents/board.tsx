import { useStore } from '@nanostores/react'
import { useMemo } from 'react'

import { useI18n } from '@/i18n'
import type { TodoItem } from '@/lib/todos'
import { $sessions } from '@/store/session'
import { $subagentsBySession, type SubagentProgress } from '@/store/subagents'
import { $todosBySession } from '@/store/todos'

export type BoardTone = 'done' | 'live' | 'next'

export interface BoardCard {
  /** Stable across renders so React never remounts a row it already shows. */
  id: string
  owner: null | string
  session: string
  text: string
  tone: BoardTone
}

const LIVE_SUBAGENT: ReadonlySet<string> = new Set(['queued', 'running'])

/**
 * One row per unit of work, from the stores the app already keeps: live and
 * terminal subagents, plus every session's todo list. Nothing here invents a
 * row — an empty store means an empty board.
 */
export function buildBoard(
  subagentsBySession: Readonly<Record<string, SubagentProgress[]>>,
  todosBySession: Readonly<Record<string, TodoItem[]>>,
  sessionTitle: (sessionId: string) => string
): BoardCard[] {
  const cards: BoardCard[] = []

  for (const [sessionId, list] of Object.entries(subagentsBySession)) {
    for (const agent of list ?? []) {
      const live: boolean = LIVE_SUBAGENT.has(agent.status)
      const tone: BoardTone = live ? 'live' : 'done'

      cards.push({
        id: `agent:${sessionId}:${agent.id}`,
        owner: agent.model ?? null,
        session: sessionTitle(sessionId),
        text: agent.goal,
        tone
      })
    }
  }

  for (const [sessionId, list] of Object.entries(todosBySession)) {
    for (const todo of list ?? []) {
      cards.push({
        id: `todo:${sessionId}:${todo.id}`,
        owner: null,
        session: sessionTitle(sessionId),
        text: todo.content,
        tone: todo.status === 'completed' ? 'done' : todo.status === 'in_progress' ? 'live' : 'next'
      })
    }
  }

  return cards
}

/** Terminal rows are history: cap them so the board stays a working surface. */
export function boardColumns(cards: readonly BoardCard[], doneLimit = 6): Record<BoardTone, BoardCard[]> {
  return {
    live: cards.filter(card => card.tone === 'live'),
    next: cards.filter(card => card.tone === 'next'),
    done: cards.filter(card => card.tone === 'done').slice(0, doneLimit)
  }
}

const TONE_CLASS: Record<BoardTone, string> = {
  done: 'text-muted-foreground/70',
  live: 'text-foreground',
  next: 'text-muted-foreground'
}

function Column({ cards, title }: { cards: BoardCard[]; title: string }) {
  if (cards.length === 0) {
    return null
  }

  return (
    <div className="min-w-0 flex-1">
      <div className="flex items-baseline justify-between gap-2 pb-1">
        <h4 className="text-[0.7rem] font-semibold tracking-wide text-muted-foreground uppercase">{title}</h4>
        <span className="text-[0.7rem] text-muted-foreground/70 tabular-nums">{cards.length}</span>
      </div>
      <ul className="grid gap-1">
        {cards.map(card => (
          <li className="min-w-0 rounded-[4px] px-2 py-1 hover:bg-(--chrome-action-hover)" key={card.id}>
            <p className={`truncate text-[0.8125rem] ${TONE_CLASS[card.tone]}`} title={card.text}>
              {card.text}
            </p>
            <p className="truncate text-[0.7rem] text-muted-foreground/70">
              {card.session}
              {card.owner ? ` · ${card.owner}` : ''}
            </p>
          </li>
        ))}
      </ul>
    </div>
  )
}

/** Cross-session work board: what is running, what is queued, what finished. */
export function TaskBoard() {
  const { t } = useI18n()
  const a = t.agents
  const subagentsBySession = useStore($subagentsBySession)
  const todosBySession = useStore($todosBySession)
  const sessions = useStore($sessions)

  const cards = useMemo(
    () =>
      buildBoard(subagentsBySession, todosBySession, sessionId => {
        const session = sessions.find(row => row.id === sessionId)

        return session?.title?.trim() || a.boardSessionUnknown
      }),
    [subagentsBySession, todosBySession, sessions, a.boardSessionUnknown]
  )

  const columns = useMemo(() => boardColumns(cards), [cards])

  if (cards.length === 0) {
    return <p className="pb-4 text-[0.75rem] text-muted-foreground">{a.boardEmpty}</p>
  }

  return (
    <section aria-label={a.board} className="border-b border-border pb-4">
      <div className="flex items-baseline justify-between gap-3 pb-2">
        <h3 className="text-[0.8125rem] font-semibold">{a.board}</h3>
        <span className="text-[0.7rem] text-muted-foreground">{a.boardItems(cards.length)}</span>
      </div>
      <div className="flex gap-6">
        <Column cards={columns.live} title={a.boardLive} />
        <Column cards={columns.next} title={a.boardNext} />
        <Column cards={columns.done} title={a.boardDone} />
      </div>
    </section>
  )
}
