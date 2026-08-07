import type { Applied, Color, Phase, Score, Seats, ServerMsg } from "./protocol";

export const EMPTY = 0;
export const BLACK = 1;
export const WHITE = 2;

/**
 * The client's view of a game.
 *
 * There is no rules engine here. Moves arrive already resolved — each carries
 * the stones it removed — so applying one is "place a stone, lift these".
 * Turn order, legality and scoring are entirely the server's business.
 */
export class GameState {
  size = $state(19);
  komi = $state(7.5);
  /** One byte per intersection. Replaced wholesale so updates are observable. */
  board = $state.raw<number[]>([]);
  toPlay = $state<Color>("black");
  moveNumber = $state(0);
  captures = $state.raw<[number, number]>([0, 0]);
  phase = $state.raw<Phase>({ state: "playing" });
  seats = $state.raw<Seats>({ black: null, white: null });
  you = $state<Color | null>(null);
  connections = $state(0);

  /** The last stone played, for the move marker. Null after a pass. */
  lastMove = $state<number | null>(null);
  /** A move we have sent but the server has not confirmed. */
  pending = $state<number | null>(null);
  /** The most recent refusal, to show the player why nothing happened. */
  rejection = $state<string | null>(null);

  // Scoring
  dead = $state.raw<Set<number>>(new Set());
  accepted = $state.raw<Color[]>([]);
  score = $state.raw<Score | null>(null);
  territory = $state.raw<number[]>([]);

  constructor() {
    this.board = new Array(this.size * this.size).fill(EMPTY);
  }

  get playing(): boolean {
    return this.phase.state === "playing";
  }

  get scoring(): boolean {
    return this.phase.state === "scoring";
  }

  get finished(): boolean {
    return this.phase.state === "finished";
  }

  /** Whether it is this player's turn to move. */
  get myTurn(): boolean {
    return this.playing && this.you !== null && this.you === this.toPlay;
  }

  get bothSeated(): boolean {
    return this.seats.black !== null && this.seats.white !== null;
  }

  /**
   * Fold a server message in.
   *
   * Returns false if the move log has a gap, meaning the caller should
   * reconnect for a fresh snapshot rather than render a wrong board.
   */
  handle(msg: ServerMsg): boolean {
    switch (msg.type) {
      case "identity":
        this.you = msg.you;
        return true;

      case "snapshot":
        this.size = msg.size;
        this.komi = msg.komi;
        this.board = msg.board.slice();
        this.toPlay = msg.to_play;
        this.moveNumber = msg.move_number;
        this.captures = [...msg.captures];
        this.phase = msg.phase;
        this.seats = msg.seats;
        this.dead = new Set(msg.dead);
        this.accepted = msg.accepted;
        this.you = msg.you;
        this.connections = msg.connections;
        this.pending = null;
        // A snapshot conveys the board, not the log, so there is no move to
        // mark. The next move played will set it.
        this.lastMove = null;
        return true;

      case "history":
        return this.#applyHistory(msg.start, msg.moves, () => {
          this.toPlay = msg.to_play;
          this.captures = [...msg.captures];
          this.phase = msg.phase;
        });

      case "seats":
        this.seats = msg.seats;
        return true;

      case "dead":
        this.dead = new Set(msg.vertices);
        this.accepted = msg.accepted;
        this.score = msg.score;
        this.territory = msg.territory;
        return true;

      case "phase_changed":
        this.phase = msg.phase;
        return true;

      case "presence":
        this.connections = msg.connections;
        return true;

      case "rejected":
        this.rejection = msg.reason;
        this.pending = null;
        return true;
    }
  }

  #applyHistory(start: number, moves: Applied[], after: () => void): boolean {
    if (start > this.moveNumber) {
      // We are missing moves between what we have and what just arrived.
      return false;
    }

    const board = this.board.slice();
    // Moves we already applied are re-sent when another client is behind.
    for (let i = this.moveNumber - start; i < moves.length; i++) {
      const applied = moves[i];
      if (applied.move.type === "play") {
        board[applied.move.vertex] = applied.color === "black" ? BLACK : WHITE;
        for (const captured of applied.captured) board[captured] = EMPTY;
        this.lastMove = applied.move.vertex;
      } else {
        this.lastMove = null;
      }
      this.moveNumber++;
    }

    this.board = board;
    this.pending = null;
    this.rejection = null;
    after();
    return true;
  }
}
