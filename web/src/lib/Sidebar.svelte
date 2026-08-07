<script lang="ts">
  import type { ConnectionStatus } from "./connection.svelte";
  import type { GameState } from "./game.svelte";
  import { blackTotal, describeResult, whiteTotal, type Color } from "./protocol";

  interface Props {
    game: GameState;
    room: string;
    status: ConnectionStatus;
    name: string;
    onclaim: (color: Color) => void;
    onrelease: () => void;
    onpass: () => void;
    onresign: () => void;
    onaccept: () => void;
    onresume: () => void;
    onrename: (name: string) => void;
  }

  let {
    game,
    room,
    status,
    name,
    onclaim,
    onrelease,
    onpass,
    onresign,
    onaccept,
    onresume,
    onrename,
  }: Props = $props();

  let copied = $state(false);

  const shareUrl = $derived(`${window.location.origin}${window.location.pathname}#${room}`);
  const canRelease = $derived(game.you !== null && game.moveNumber === 0);
  const iAccepted = $derived(game.you !== null && game.accepted.includes(game.you));

  async function copyLink() {
    try {
      await navigator.clipboard.writeText(shareUrl);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      // Clipboard access can be denied; the input is selectable as a fallback.
      copied = false;
    }
  }

  function seatLabel(color: Color): string {
    const occupant = color === "black" ? game.seats.black : game.seats.white;
    if (occupant) return occupant;
    return "Empty seat";
  }
</script>

<aside class="sidebar">
  <header>
    <h1>igo</h1>
    <span class="status" class:ok={status === "connected"}>
      {status}
      {#if game.connections > 0}· {game.connections} here{/if}
    </span>
  </header>

  <section class="share">
    <label for="share">Invite your opponent</label>
    <div class="row">
      <input id="share" readonly value={shareUrl} onclick={(e) => e.currentTarget.select()} />
      <button onclick={copyLink}>{copied ? "Copied" : "Copy"}</button>
    </div>
  </section>

  <section class="seats">
    {#each ["black", "white"] as const as color (color)}
      <div class="seat" class:mine={game.you === color} class:turn={game.playing && game.toPlay === color}>
        <span class="disc {color}"></span>
        <div class="who">
          <strong>{seatLabel(color)}</strong>
          <small>
            {game.captures[color === "black" ? 0 : 1]} captured
            {#if color === "white"}· {game.komi} komi{/if}
          </small>
        </div>
        {#if (color === "black" ? game.seats.black : game.seats.white) === null && game.you === null}
          <button onclick={() => onclaim(color)}>Sit</button>
        {:else if game.you === color && canRelease}
          <button class="quiet" onclick={onrelease}>Leave</button>
        {/if}
      </div>
    {/each}
  </section>

  {#if game.you === null}
    <p class="note">You are watching. Take a seat to play.</p>
  {:else if !game.bothSeated}
    <p class="note">Waiting for an opponent — send them the link.</p>
  {/if}

  {#if game.playing && game.bothSeated && game.you !== null}
    <section class="actions">
      <button onclick={onpass} disabled={!game.myTurn}>Pass</button>
      <button class="danger" onclick={onresign}>Resign</button>
    </section>
    <p class="note">
      {game.myTurn ? "Your move." : "Waiting for your opponent."}
    </p>
  {/if}

  {#if game.scoring}
    <section class="scoring">
      <h2>Scoring</h2>
      <p class="note">
        Click a group to mark it dead. Both players must agree.
      </p>
      {#if game.score}
        <table>
          <tbody>
            <tr>
              <th>Black</th>
              <td>{game.score.black_stones} + {game.score.black_territory}</td>
              <td class="total">{blackTotal(game.score)}</td>
            </tr>
            <tr>
              <th>White</th>
              <td>{game.score.white_stones} + {game.score.white_territory} + {game.score.komi}</td>
              <td class="total">{whiteTotal(game.score)}</td>
            </tr>
          </tbody>
        </table>
      {/if}
      {#if game.you !== null}
        <div class="actions">
          <button onclick={onaccept} disabled={iAccepted}>
            {iAccepted ? "Waiting for opponent" : "Accept"}
          </button>
          <button class="quiet" onclick={onresume}>Resume play</button>
        </div>
      {/if}
      {#if game.accepted.length > 0}
        <p class="note">Accepted: {game.accepted.join(", ")}</p>
      {/if}
    </section>
  {/if}

  {#if game.phase.state === "finished"}
    <section class="result">
      <h2>{describeResult(game.phase.result)}</h2>
      <a href="/api/sgf/{room}" download="{room}.sgf">Download game record</a>
    </section>
  {/if}

  {#if game.rejection}
    <p class="rejection" role="status">{game.rejection}</p>
  {/if}

  <section class="you">
    <label for="name">Your name</label>
    <input
      id="name"
      value={name}
      onchange={(e) => onrename(e.currentTarget.value)}
      maxlength="40"
    />
  </section>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    min-width: 17rem;
    max-width: 22rem;
  }

  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
  }
  h1 {
    margin: 0;
    font-size: 1.4rem;
    letter-spacing: 0.04em;
  }
  h2 {
    margin: 0 0 0.4rem;
    font-size: 1rem;
  }

  .status {
    font-size: 0.75rem;
    color: var(--muted);
    text-transform: lowercase;
  }
  .status.ok {
    color: var(--good);
  }

  label {
    display: block;
    font-size: 0.75rem;
    color: var(--muted);
    margin-bottom: 0.3rem;
  }

  .row {
    display: flex;
    gap: 0.4rem;
  }
  input {
    flex: 1;
    min-width: 0;
    padding: 0.4rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--surface);
    color: inherit;
    font: inherit;
    font-size: 0.85rem;
  }

  button {
    padding: 0.4rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--surface);
    color: inherit;
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    border-color: var(--accent);
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  button.danger:hover:not(:disabled) {
    border-color: var(--bad);
    color: var(--bad);
  }
  button.quiet {
    color: var(--muted);
  }

  .seats {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .seat {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .seat.mine {
    border-color: var(--accent);
  }
  /* Whose turn it is has to be readable at a glance from across the room. */
  .seat.turn {
    background: var(--surface-strong);
  }
  .seat .who {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .seat strong {
    font-size: 0.9rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .seat small {
    font-size: 0.72rem;
    color: var(--muted);
  }

  .disc {
    width: 1.1rem;
    height: 1.1rem;
    border-radius: 50%;
    flex: none;
  }
  .disc.black {
    background: #1a1a1c;
  }
  .disc.white {
    background: #f7f5ef;
    border: 1px solid var(--border);
  }

  .actions {
    display: flex;
    gap: 0.5rem;
  }
  .actions button {
    flex: 1;
  }

  .note {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .rejection {
    margin: 0;
    padding: 0.5rem 0.6rem;
    border-radius: 5px;
    background: var(--bad-bg);
    color: var(--bad);
    font-size: 0.8rem;
  }

  .scoring,
  .result {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.82rem;
  }
  th {
    text-align: left;
    font-weight: 600;
  }
  td {
    color: var(--muted);
  }
  td.total {
    text-align: right;
    color: inherit;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  a {
    color: var(--accent);
    font-size: 0.85rem;
  }
</style>
