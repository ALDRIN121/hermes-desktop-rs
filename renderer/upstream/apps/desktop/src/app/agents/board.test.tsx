import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, expect, it } from 'vitest'

import type { TodoItem } from '@/lib/todos'
import { $sessions } from '@/store/session'
import { $subagentsBySession, type SubagentProgress } from '@/store/subagents'
import { $todosBySession } from '@/store/todos'

import { boardColumns, buildBoard, TaskBoard } from './board'

afterEach(() => {
  cleanup()
  $subagentsBySession.set({})
  $todosBySession.set({})
  $sessions.set([])
})

const agent = (over: Partial<SubagentProgress>): SubagentProgress =>
  ({
    goal: 'Review the schema',
    id: 'a1',
    parentId: null,
    startedAt: 0,
    status: 'running',
    taskCount: 1,
    taskIndex: 1,
    updatedAt: 0,
    ...over
  }) as SubagentProgress

const todo = (over: Partial<TodoItem>): TodoItem => ({ content: 'Write the migration', id: 't1', status: 'pending', ...over })

it('builds one row per unit of work from the stores the app already keeps', () => {
  const cards = buildBoard(
    { s1: [agent({ id: 'a1', status: 'running' }), agent({ goal: 'Ship it', id: 'a2', status: 'completed' })] },
    { s1: [todo({ id: 't1', status: 'in_progress' }), todo({ id: 't2', status: 'pending' }), todo({ content: 'Done thing', id: 't3', status: 'completed' })] },
    () => 'Session one'
  )

  expect(cards).toHaveLength(5)
  expect(cards.map(card => card.tone)).toEqual(['live', 'done', 'live', 'next', 'done'])
  expect(cards.every(card => card.session === 'Session one')).toBe(true)
})

it('caps finished work so the board stays a working surface', () => {
  const cards = buildBoard({}, { s1: Array.from({ length: 9 }, (_, index) => todo({ id: `t${index}`, status: 'completed' })) }, () => 'S')

  expect(boardColumns(cards, 4).done).toHaveLength(4)
})

it('renders the three columns with session names and an honest empty state', () => {
  $sessions.set([{ id: 's1', title: 'Voice work' } as never])
  $subagentsBySession.set({ s1: [agent({ goal: 'Review the schema', status: 'running' })] })
  $todosBySession.set({ s1: [todo({ content: 'Write the migration', status: 'pending' })] })

  render(<TaskBoard />)

  expect(screen.getByText('Tasks & agents')).toBeTruthy()
  expect(screen.getByText('In progress')).toBeTruthy()
  expect(screen.getByText('Queued')).toBeTruthy()
  expect(screen.getByText('Review the schema')).toBeTruthy()
  expect(screen.getByText('Write the migration')).toBeTruthy()
  expect(screen.getAllByText('Voice work')).toHaveLength(2)
  expect(screen.getByText('2 items')).toBeTruthy()

  cleanup()
  $subagentsBySession.set({})
  $todosBySession.set({})
  render(<TaskBoard />)

  expect(screen.getByText('No queued or running work in any session.')).toBeTruthy()
  expect(screen.queryByText('In progress')).toBeNull()
})
