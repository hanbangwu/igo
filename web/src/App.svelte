<script lang="ts">
  import Board from './lib/Board.svelte'
  import Sidebar from './lib/Sidebar.svelte'
  import { Connection } from './lib/connection.svelte'
  import { GameState } from './lib/game.svelte'
  import {
    onRoomChange,
    playerName,
    playerToken,
    roomId,
    setPlayerName,
    socketUrl
  } from './lib/identity'
  import type { Color, ClientMsg } from './lib/protocol'

  let room = $state(roomId())
  let name = $state(playerName())
  let game = $state(new GameState())
  let connection = $state<Connection | null>(null)

  const token = playerToken()
  const status = $derived(connection?.status ?? 'connecting')

  $effect(() => onRoomChange((id) => (room = id)))

  $effect(() => {
    const current = room
    const state = new GameState()
    game = state

    const conn = new Connection({
      url: socketUrl(current),
      onOpen: () => {
        conn.send({ type: 'hello', token, name })
      },
      onMessage: (msg) => {
        if (!state.handle(msg)) {
          conn.resync()
        }
      }
    })
    connection = conn

    return () => {
      conn.dispose()
      connection = null
    }
  })

  function send(msg: ClientMsg) {
    connection?.send(msg)
  }

  function play(vertex: number) {
    game.pending = vertex
    game.rejection = null
    send({
      type: 'play',
      move_number: game.moveNumber,
      move: { type: 'play', vertex }
    })
  }

  function rename(next: string) {
    const trimmed = next.trim() || 'Anonymous'
    name = trimmed
    setPlayerName(trimmed)
    send({ type: 'hello', token, name: trimmed })
  }
</script>

<main>
  <div class="board-area">
    <Board
      size={game.size}
      board={game.board}
      lastMove={game.lastMove}
      pending={game.pending}
      dead={game.dead}
      territory={game.territory}
      scoring={game.scoring}
      myTurn={game.myTurn && game.bothSeated}
      myColor={game.you}
      onplay={play}
      ontoggledead={(vertex) => send({ type: 'toggle_dead', vertex })}
    />
  </div>

  <Sidebar
    {game}
    {room}
    {status}
    {name}
    onclaim={(color: Color) => send({ type: 'claim_seat', color })}
    onrelease={() => send({ type: 'release_seat' })}
    onpass={() =>
      send({
        type: 'play',
        move_number: game.moveNumber,
        move: { type: 'pass' }
      })}
    onresign={() => {
      if (confirm('Resign this game?')) {
        send({
          type: 'play',
          move_number: game.moveNumber,
          move: { type: 'resign' }
        })
      }
    }}
    onaccept={() => send({ type: 'accept_score' })}
    onresume={() => send({ type: 'resume_play' })}
    onrename={rename}
  />
</main>

{#if status === 'desynchronized'}
  <div class="banner" role="alert">Lost contact with the server. Reload the page to continue.</div>
{/if}

<style>
  main {
    display: flex;
    gap: 2rem;
    align-items: flex-start;
    padding: 1.5rem;
    max-width: 72rem;
    margin: 0 auto;
  }

  .board-area {
    flex: 1;
    display: flex;
    justify-content: center;
    min-width: 0;
  }

  .banner {
    position: fixed;
    inset: auto 0 0 0;
    padding: 0.75rem 1rem;
    text-align: center;
    background: var(--bad-bg);
    color: var(--bad);
    font-size: 0.9rem;
  }

  @media (max-width: 60rem) {
    main {
      flex-direction: column;
      align-items: stretch;
      gap: 1.25rem;
      padding: 1rem;
    }
  }
</style>
