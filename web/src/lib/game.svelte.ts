import type { Applied, Color, Phase, Score, Seats, ServerMsg } from './protocol'

export const EMPTY = 0
export const BLACK = 1
export const WHITE = 2

export class GameState {
  size = $state(19)
  komi = $state(7.5)
  board = $state.raw<number[]>([])
  toPlay = $state<Color>('black')
  moveNumber = $state(0)
  captures = $state.raw<[number, number]>([0, 0])
  phase = $state.raw<Phase>({ state: 'playing' })
  seats = $state.raw<Seats>({ black: null, white: null })
  you = $state<Color | null>(null)
  connections = $state(0)

  lastMove = $state<number | null>(null)
  pending = $state<number | null>(null)
  rejection = $state<string | null>(null)

  dead = $state.raw<Set<number>>(new Set())
  accepted = $state.raw<Color[]>([])
  score = $state.raw<Score | null>(null)
  territory = $state.raw<number[]>([])

  constructor() {
    this.board = new Array(this.size * this.size).fill(EMPTY)
  }

  get playing(): boolean {
    return this.phase.state === 'playing'
  }

  get scoring(): boolean {
    return this.phase.state === 'scoring'
  }

  get finished(): boolean {
    return this.phase.state === 'finished'
  }

  get myTurn(): boolean {
    return this.playing && this.you !== null && this.you === this.toPlay
  }

  get bothSeated(): boolean {
    return this.seats.black !== null && this.seats.white !== null
  }

  handle(msg: ServerMsg): boolean {
    switch (msg.type) {
      case 'identity':
        this.you = msg.you
        return true

      case 'snapshot':
        this.size = msg.size
        this.komi = msg.komi
        this.board = msg.board.slice()
        this.toPlay = msg.to_play
        this.moveNumber = msg.move_number
        this.captures = [...msg.captures]
        this.phase = msg.phase
        this.seats = msg.seats
        this.dead = new Set(msg.dead)
        this.accepted = msg.accepted
        this.you = msg.you
        this.connections = msg.connections
        this.pending = null
        this.lastMove = null
        return true

      case 'history':
        return this.#applyHistory(msg.start, msg.moves, () => {
          this.toPlay = msg.to_play
          this.captures = [...msg.captures]
          this.phase = msg.phase
        })

      case 'seats':
        this.seats = msg.seats
        return true

      case 'dead':
        this.dead = new Set(msg.vertices)
        this.accepted = msg.accepted
        this.score = msg.score
        this.territory = msg.territory
        return true

      case 'phase_changed':
        this.phase = msg.phase
        return true

      case 'presence':
        this.connections = msg.connections
        return true

      case 'rejected':
        this.rejection = msg.reason
        this.pending = null
        return true
    }
  }

  #applyHistory(start: number, moves: Applied[], after: () => void): boolean {
    if (start > this.moveNumber) {
      return false
    }

    const board = this.board.slice()
    for (let i = this.moveNumber - start; i < moves.length; i++) {
      const applied = moves[i]
      if (applied.move.type === 'play') {
        board[applied.move.vertex] = applied.color === 'black' ? BLACK : WHITE
        for (const captured of applied.captured) board[captured] = EMPTY
        this.lastMove = applied.move.vertex
      } else {
        this.lastMove = null
      }
      this.moveNumber++
    }

    this.board = board
    this.pending = null
    this.rejection = null
    after()
    return true
  }
}
