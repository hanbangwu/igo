<script lang="ts">
  import { BLACK, EMPTY, WHITE } from './game.svelte'
  import type { Color } from './protocol'

  interface Props {
    size: number
    board: number[]
    lastMove: number | null
    pending: number | null
    dead: Set<number>
    territory: number[]
    scoring: boolean
    myTurn: boolean
    myColor: Color | null
    onplay: (vertex: number) => void
    ontoggledead: (vertex: number) => void
  }

  let {
    size,
    board,
    lastMove,
    pending,
    dead,
    territory,
    scoring,
    myTurn,
    myColor,
    onplay,
    ontoggledead
  }: Props = $props()

  let hover = $state<number | null>(null)
  let keyboardCursor = $state<number | null>(null)

  const indices = $derived(Array.from({ length: size }, (_, i) => i))
  const stars = $derived(starPoints(size))

  const x = (v: number) => (v % size) + 1
  const y = (v: number) => Math.floor(v / size) + 1

  function starPoints(n: number): number[] {
    if (n < 7) return []
    const edge = n >= 13 ? 3 : 2
    const middle = (n - 1) / 2
    const coords = n % 2 === 1 && n >= 9 ? [edge, middle, n - 1 - edge] : [edge, n - 1 - edge]
    return coords.flatMap((row) => coords.map((col) => row * n + col))
  }

  function vertexAt(event: MouseEvent | PointerEvent): number | null {
    const svg = event.currentTarget as SVGSVGElement
    const rect = svg.getBoundingClientRect()
    const unit = rect.width / (size + 1)
    const col = Math.round((event.clientX - rect.left) / unit - 1)
    const row = Math.round((event.clientY - rect.top) / unit - 1)
    if (col < 0 || col >= size || row < 0 || row >= size) return null
    return row * size + col
  }

  function activate(vertex: number | null) {
    if (vertex === null) return
    if (scoring) {
      if (board[vertex] !== EMPTY) ontoggledead(vertex)
      return
    }
    if (!myTurn || board[vertex] !== EMPTY) return
    onplay(vertex)
  }

  function onKeyDown(event: KeyboardEvent) {
    const cursor = keyboardCursor ?? Math.floor((size * size) / 2)
    const deltas: Record<string, number> = {
      ArrowLeft: -1,
      ArrowRight: 1,
      ArrowUp: -size,
      ArrowDown: size
    }

    if (event.key in deltas) {
      event.preventDefault()
      const next = cursor + deltas[event.key]
      const sameRow = Math.floor(next / size) === Math.floor(cursor / size)
      const horizontal = event.key === 'ArrowLeft' || event.key === 'ArrowRight'
      if (next >= 0 && next < size * size && (!horizontal || sameRow)) {
        keyboardCursor = next
      } else {
        keyboardCursor = cursor
      }
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      activate(keyboardCursor ?? cursor)
    }
  }

  const ghost = $derived(
    !scoring && myTurn && hover !== null && board[hover] === EMPTY ? hover : null
  )
  const cursor = $derived(keyboardCursor)
</script>

<svg
  class="goban"
  class:scoring
  viewBox="0 0 {size + 1} {size + 1}"
  role="application"
  aria-label="Go board, {size} by {size}"
  tabindex="0"
  onpointermove={(e) => (hover = vertexAt(e))}
  onpointerleave={() => (hover = null)}
  onclick={(e) => activate(vertexAt(e))}
  onkeydown={onKeyDown}
  onblur={() => (keyboardCursor = null)}
>
  <rect class="wood" x="0" y="0" width={size + 1} height={size + 1} rx="0.25" />

  {#each indices as i (i)}
    <line class="grid" x1="1" y1={i + 1} x2={size} y2={i + 1} />
    <line class="grid" x1={i + 1} y1="1" x2={i + 1} y2={size} />
  {/each}

  {#each stars as v (v)}
    <circle class="star" cx={x(v)} cy={y(v)} r="0.09" />
  {/each}

  {#if scoring && territory.length === board.length}
    {#each board as _point, v (v)}
      {#if board[v] === EMPTY && territory[v] !== 0}
        <rect
          class="territory"
          class:black={territory[v] === BLACK}
          class:white={territory[v] === WHITE}
          x={x(v) - 0.18}
          y={y(v) - 0.18}
          width="0.36"
          height="0.36"
        />
      {/if}
    {/each}
  {/if}

  {#each board as point, v (v)}
    {#if point !== EMPTY}
      <circle
        class="stone"
        class:black={point === BLACK}
        class:white={point === WHITE}
        class:dead={dead.has(v)}
        cx={x(v)}
        cy={y(v)}
        r="0.47"
      />
    {/if}
  {/each}

  {#if lastMove !== null && board[lastMove] !== EMPTY}
    <circle
      class="last-move"
      class:on-black={board[lastMove] === BLACK}
      cx={x(lastMove)}
      cy={y(lastMove)}
      r="0.16"
    />
  {/if}

  {#if pending !== null}
    <circle
      class="stone pending"
      class:black={myColor === 'black'}
      class:white={myColor === 'white'}
      cx={x(pending)}
      cy={y(pending)}
      r="0.47"
    />
  {/if}

  {#if ghost !== null}
    <circle
      class="stone ghost"
      class:black={myColor === 'black'}
      class:white={myColor === 'white'}
      cx={x(ghost)}
      cy={y(ghost)}
      r="0.47"
    />
  {/if}

  {#if cursor !== null}
    <rect class="cursor" x={x(cursor) - 0.5} y={y(cursor) - 0.5} width="1" height="1" />
  {/if}
</svg>

<style>
  .goban {
    width: 100%;
    height: auto;
    max-width: min(78vh, 100%);
    display: block;
    border-radius: 6px;
    touch-action: manipulation;
    cursor: default;
  }
  .goban:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 3px;
  }

  .wood {
    fill: var(--board);
  }
  .grid {
    stroke: var(--board-line);
    stroke-width: 0.035;
    stroke-linecap: round;
  }
  .star {
    fill: var(--board-line);
  }

  .stone.black {
    fill: #1a1a1c;
  }
  .stone.white {
    fill: #f7f5ef;
    stroke: #56504420;
    stroke-width: 0.03;
  }
  .stone.dead {
    opacity: 0.3;
  }

  .stone.ghost {
    opacity: 0.35;
    pointer-events: none;
  }
  .stone.pending {
    opacity: 0.6;
  }

  .last-move {
    fill: none;
    stroke: #1a1a1c;
    stroke-width: 0.07;
  }
  .last-move.on-black {
    stroke: #f7f5ef;
  }

  .territory.black {
    fill: #1a1a1c;
    opacity: 0.55;
  }
  .territory.white {
    fill: #f7f5ef;
    opacity: 0.75;
    stroke: #00000030;
    stroke-width: 0.02;
  }

  .cursor {
    fill: none;
    stroke: var(--accent);
    stroke-width: 0.06;
  }

  .goban.scoring {
    cursor: pointer;
  }
</style>
