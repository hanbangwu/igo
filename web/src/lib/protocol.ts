/**
 * Mirrors of the Rust wire types in `igo-server/src/protocol.rs`.
 *
 * These are hand-written, so they can drift. If a field is renamed on the Rust
 * side without being changed here, `svelte-check` will not catch it — the
 * server tests will.
 */

export type Color = "black" | "white";

export type Move =
  | { type: "play"; vertex: number }
  | { type: "pass" }
  | { type: "resign" };

export type Score = {
  black_stones: number;
  black_territory: number;
  white_stones: number;
  white_territory: number;
  dame: number;
  komi: number;
};

export type GameResult =
  | { kind: "counted"; winner: Color | null; score: Score }
  | { kind: "resignation"; winner: Color };

export type Phase =
  | { state: "playing" }
  | { state: "scoring" }
  | { state: "finished"; result: GameResult };

export type Applied = {
  move_number: number;
  color: Color;
  move: Move;
  /** Stones this move removed. Lets the client apply a move as a pure delta. */
  captured: number[];
};

export type Seats = { black: string | null; white: string | null };

export type Snapshot = {
  size: number;
  komi: number;
  /** One byte per intersection: 0 empty, 1 black, 2 white. */
  board: number[];
  to_play: Color;
  move_number: number;
  captures: [number, number];
  phase: Phase;
  seats: Seats;
  dead: number[];
  accepted: Color[];
  you: Color | null;
  connections: number;
};

export type ServerMsg =
  | { type: "identity"; conn: number; you: Color | null }
  | ({ type: "snapshot" } & Snapshot)
  | {
      type: "history";
      start: number;
      moves: Applied[];
      to_play: Color;
      captures: [number, number];
      phase: Phase;
    }
  | { type: "seats"; seats: Seats }
  | {
      type: "dead";
      vertices: number[];
      accepted: Color[];
      score: Score;
      /** Owner of each point: 0 neither, 1 black, 2 white. */
      territory: number[];
    }
  | { type: "phase_changed"; phase: Phase }
  | { type: "presence"; connections: number }
  | { type: "rejected"; reason: string };

export type ClientMsg =
  | { type: "hello"; token: string; name: string }
  | { type: "claim_seat"; color: Color }
  | { type: "release_seat" }
  | { type: "play"; move_number: number; move: Move }
  | { type: "toggle_dead"; vertex: number }
  | { type: "accept_score" }
  | { type: "resume_play" };

/** Black's total under area scoring. */
export function blackTotal(score: Score): number {
  return score.black_stones + score.black_territory;
}

/** White's total, including komi. */
export function whiteTotal(score: Score): number {
  return score.white_stones + score.white_territory + score.komi;
}

export function describeResult(result: GameResult): string {
  if (result.kind === "resignation") {
    return `${result.winner === "black" ? "Black" : "White"} wins by resignation`;
  }
  if (result.winner === null) {
    return "Draw";
  }
  const margin = Math.abs(blackTotal(result.score) - whiteTotal(result.score));
  const name = result.winner === "black" ? "Black" : "White";
  return `${name} wins by ${margin}`;
}
