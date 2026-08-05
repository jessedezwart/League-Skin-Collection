<script lang="ts">
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import SkinCard from "$lib/SkinCard.svelte";
  import {
    formatMoney,
    formatAvailabilityDate,
    rarityLabel,
    SORTERS,
    SORT_LABELS,
    type Champion,
    type ConnectionStatus,
    type Currency,
    type Skin,
    type SortKey,
  } from "$lib/types";

  let status = $state<ConnectionStatus>({ state: "notRunning" });
  let skins = $state<Skin[] | null>(null);
  let champions = $state<Champion[]>([]);
  let champSort = $state<"skins" | "value" | "name">("skins");
  let selectedChampion = $state<number | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  let search = $state("");
  let sortBy = $state<SortKey>("value");
  let legacyOnly = $state(false);
  let currency = $state<Currency>(
    (localStorage.getItem("currency") as Currency) || "EUR"
  );
  $effect(() => localStorage.setItem("currency", currency));

  let modalSkin = $state<Skin | null>(null);
  let modalSrc = $state<string | null>(null);

  onMount(() => {
    const poll = async () => {
      try {
        status = await invoke<ConnectionStatus>("lcu_status");
      } catch {
        status = { state: "notRunning" };
      }
      if (status.state === "connected" && skins === null && !loading) {
        void load();
      }
    };
    void poll();
    const id = setInterval(poll, 4000);
    return () => clearInterval(id);
  });

  async function load() {
    loading = true;
    error = null;
    try {
      skins = await invoke<Skin[]>("fetch_collection");
      champions = await invoke<Champion[]>("list_champions");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  const champStats = $derived.by(() => {
    const stats = new Map<number, { count: number; rp: number }>();
    for (const s of skins ?? []) {
      const cur = stats.get(s.championId) ?? { count: 0, rp: 0 };
      cur.count += 1;
      cur.rp += s.rp ?? 0;
      stats.set(s.championId, cur);
    }
    return stats;
  });

  const sortedChampions = $derived.by(() => {
    const stat = (c: Champion) =>
      champStats.get(c.championId) ?? { count: 0, rp: 0 };
    const bySort: Record<typeof champSort, (a: Champion, b: Champion) => number> = {
      skins: (a, b) =>
        stat(b).count - stat(a).count || a.name.localeCompare(b.name),
      value: (a, b) => stat(b).rp - stat(a).rp || a.name.localeCompare(b.name),
      name: (a, b) => a.name.localeCompare(b.name),
    };
    return [...champions].sort(bySort[champSort]);
  });

  const skinlessCount = $derived(
    champions.filter((c) => !champStats.has(c.championId)).length
  );

  const filtered = $derived.by(() => {
    if (!skins) return [];
    const q = search.trim().toLowerCase();
    let list = skins.filter(
      (s) =>
        (!legacyOnly || s.isLegacy) &&
        (selectedChampion === null || s.championId === selectedChampion) &&
        (!q ||
          s.name.toLowerCase().includes(q) ||
          s.championName.toLowerCase().includes(q))
    );
    // Newest-first is the tiebreaker for every sort, so equal-value or
    // same-champion runs read as most recent first.
    const primary = SORTERS[sortBy];
    return [...list].sort(
      (a, b) =>
        primary(a, b) || SORTERS.availability(a, b) || a.skinId - b.skinId
    );
  });

  const totalRp = $derived(
    (skins ?? []).reduce((sum, s) => sum + (s.rp ?? 0), 0)
  );
  const estimatedCount = $derived(
    (skins ?? []).filter((s) => s.valueSource === "estimate").length
  );

  async function openModal(skin: Skin) {
    modalSkin = skin;
    modalSrc = null;
    try {
      const path = await invoke<string>("get_skin_image", {
        skinId: skin.skinId,
        kind: "splash",
      });
      modalSrc = convertFileSrc(path);
    } catch {
      modalSrc = null;
    }
  }

  function closeModal() {
    modalSkin = null;
    modalSrc = null;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && closeModal()} />

<div class="app">
  <header>
    <div class="brand">
      <h1>Skin Collection</h1>
      <span class="status {status.state}">
        {#if status.state === "connected"}
          ● {status.summonerName || "Connected"}
        {:else if status.state === "notLoggedIn"}
          ● Client starting…
        {:else}
          ● League client not running
        {/if}
      </span>
    </div>
    {#if skins}
      <div class="totals">
        <div class="stat">
          <span class="num">{skins.length}</span>
          <span class="label">skins</span>
        </div>
        <div class="stat">
          <span class="num gold">{totalRp.toLocaleString()} RP</span>
          <span class="label">total value</span>
        </div>
        <div
          class="stat"
          title="Money values assume the base RP rate of the smallest package ({currency === 'EUR' ? '€' : '$'}4.99 = 575 RP, no bonus RP). Larger packages include bonus RP, so buying this collection would cost less in practice.{estimatedCount ? ` ${estimatedCount} skin${estimatedCount === 1 ? '' : 's'} use an estimated RP value based on rarity (legacy/vaulted, not in the current store); estimates are shown with a '~' prefix.` : ''}"
        >
          <span class="num"
            >{formatMoney(totalRp, currency)}
            <svg class="info-icon" viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
              <circle cx="8" cy="8" r="7" fill="none" stroke="currentColor" stroke-width="1.3" />
              <circle cx="8" cy="4.7" r="0.9" fill="currentColor" />
              <rect x="7.15" y="7" width="1.7" height="5.2" rx="0.6" fill="currentColor" />
            </svg></span
          >
          <span class="label money-label">
            <button
              class="currency-toggle"
              onclick={() => (currency = currency === "EUR" ? "USD" : "EUR")}
              title="Switch currency"
            >
              {currency} ⇄
            </button>
          </span>
        </div>
      </div>
    {/if}
  </header>

  {#if skins}
    <div class="controls">
      <input
        type="search"
        placeholder="Search skin or champion…"
        bind:value={search}
      />
      <select bind:value={sortBy} aria-label="Sort by">
        {#each Object.entries(SORT_LABELS) as [key, label]}
          <option value={key}>Sort: {label}</option>
        {/each}
      </select>
      <label class="check">
        <input type="checkbox" bind:checked={legacyOnly} />
        Legacy only
      </label>
      <button class="refresh" onclick={load} disabled={loading}>Refresh</button>
    </div>
  {/if}

  <div class="layout">
    {#if skins && champions.length}
      <aside class="champs">
        <div class="champs-head">
          <select bind:value={champSort}>
            <option value="skins">Sort: Skins</option>
            <option value="value">Sort: Value</option>
            <option value="name">Sort: Name</option>
          </select>
          <span class="skinless">{skinlessCount} without skins</span>
        </div>
        <div class="champs-list">
          <button
            class="champ-row all"
            class:selected={selectedChampion === null}
            onclick={() => (selectedChampion = null)}
          >
            <span class="champ-name">All champions</span>
            <span class="champ-count">{skins.length}</span>
          </button>
          {#each sortedChampions as champ (champ.championId)}
            {@const stat = champStats.get(champ.championId)}
            <button
              class="champ-row"
              class:none={!stat}
              class:selected={selectedChampion === champ.championId}
              onclick={() =>
                (selectedChampion =
                  selectedChampion === champ.championId
                    ? null
                    : champ.championId)}
            >
              <span class="champ-name">{champ.name}</span>
              {#if stat}
                <span class="champ-value">
                  {stat.rp.toLocaleString()} ({formatMoney(stat.rp, currency)})
                </span>
                <span class="champ-count">{stat.count}</span>
              {:else}
                <span class="champ-count zero">0</span>
              {/if}
            </button>
          {/each}
        </div>
      </aside>
    {/if}

    <main>
      {#if status.state !== "connected" && !skins}
        <div class="empty">
          <p class="big">Waiting for the League client…</p>
          <p>Start the League of Legends client and log in. Checking every few seconds.</p>
        </div>
      {:else if loading && !skins}
        <div class="empty"><p class="big">Loading your collection…</p></div>
      {:else if error}
        <div class="empty">
          <p class="big">Couldn't load collection</p>
          <p>{error}</p>
          <button class="refresh" onclick={load}>Retry</button>
        </div>
      {:else if skins}
        {#if filtered.length === 0}
          <div class="empty">
            <p>
              {selectedChampion !== null && !champStats.has(selectedChampion)
                ? "No owned skins for this champion."
                : "No skins match."}
            </p>
          </div>
        {:else}
          <div class="grid">
            {#each filtered as skin (skin.skinId)}
              <SkinCard {skin} {currency} onopen={openModal} />
            {/each}
          </div>
        {/if}
      {/if}
    </main>
  </div>
</div>

{#if modalSkin}
  <div
    class="overlay"
    onclick={closeModal}
    onkeydown={(e) => e.key === "Enter" && closeModal()}
    role="button"
    tabindex="0"
  >
    <div class="modal">
      {#if modalSrc}
        <img src={modalSrc} alt={modalSkin.name} />
      {:else}
        <div class="modal-loading">Loading splash…</div>
      {/if}
      <div class="modal-info">
        <div>
          <strong>{modalSkin.name}</strong>
          <span class="muted"> — {modalSkin.championName}</span>
          {#if rarityLabel(modalSkin.rarity)}
            <span class="muted"> · {rarityLabel(modalSkin.rarity)}</span>
          {/if}
          {#if modalSkin.isLegacy}<span class="muted"> · Legacy</span>{/if}
          {#if formatAvailabilityDate(modalSkin.availabilityDate)}
            <span class="muted">
              · {formatAvailabilityDate(modalSkin.availabilityDate)}</span
            >
          {/if}
        </div>
        <span class="gold">
          {#if modalSkin.rp !== null}
            {modalSkin.valueSource === "estimate" ? "~" : ""}{modalSkin.rp.toLocaleString()} RP
            <span class="muted">
              ({formatMoney(modalSkin.rp, currency)})
            </span>
          {:else}
            Not purchasable with RP
          {/if}
        </span>
      </div>
    </div>
  </div>
{/if}

<style>
  :global(:root) {
    --bg: #0b0f14;
    --panel: #121821;
    --border: #1f2a37;
    --text: #e8e6e3;
    --muted: #8a93a0;
    --gold: #c9a24b;
  }
  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font-family: "Segoe UI", Inter, Helvetica, Arial, sans-serif;
  }
  .app {
    max-width: 1400px;
    margin: 0 auto;
    padding: 20px 24px 48px;
  }
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
    font-size: 1.5rem;
    letter-spacing: 0.02em;
    color: var(--gold);
  }
  .status {
    font-size: 0.8rem;
    color: var(--muted);
  }
  .status.connected {
    color: #6fdb8b;
  }
  .totals {
    display: flex;
    gap: 24px;
  }
  .stat {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  .stat .num {
    font-size: 1.25rem;
    font-weight: 700;
  }
  .stat .label {
    font-size: 0.72rem;
    color: var(--muted);
  }
  .gold {
    color: var(--gold);
  }
  .currency-toggle {
    all: unset;
    cursor: pointer;
    color: var(--muted);
    font-size: 0.72rem;
    border-bottom: 1px dotted var(--muted);
  }
  .currency-toggle:hover {
    color: var(--gold);
    border-color: var(--gold);
  }
  .info-icon {
    color: var(--muted);
    cursor: help;
    vertical-align: middle;
    position: relative;
    top: -1px;
  }
  .info-icon:hover {
    color: var(--gold);
  }
  .muted {
    color: var(--muted);
  }

  .controls {
    display: flex;
    gap: 10px;
    align-items: center;
    margin: 18px 0 14px;
    flex-wrap: wrap;
  }
  input[type="search"],
  select {
    background: var(--panel);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 8px;
    padding: 8px 12px;
    font-size: 0.85rem;
    outline: none;
  }
  input[type="search"] {
    flex: 1;
    min-width: 200px;
  }
  input[type="search"]:focus,
  select:focus {
    border-color: var(--gold);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.85rem;
    color: var(--muted);
    user-select: none;
  }
  .refresh {
    background: var(--panel);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: 8px;
    padding: 8px 14px;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .refresh:hover:not(:disabled) {
    border-color: var(--gold);
  }
  .refresh:disabled {
    opacity: 0.5;
  }

  .layout {
    display: flex;
    gap: 16px;
    align-items: flex-start;
  }
  .champs {
    flex: 0 0 260px;
    position: sticky;
    top: 12px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
  }
  .champs-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px;
    border-bottom: 1px solid var(--border);
  }
  .champs-head select {
    padding: 5px 8px;
    font-size: 0.78rem;
  }
  .skinless {
    font-size: 0.7rem;
    color: var(--muted);
    text-align: right;
  }
  .champs-list {
    max-height: calc(100vh - 210px);
    overflow-y: auto;
  }
  .champ-row {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    padding: 5px 10px;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .champ-row:hover {
    background: rgba(201, 162, 75, 0.08);
  }
  .champ-row.selected {
    background: rgba(201, 162, 75, 0.16);
    color: var(--gold);
  }
  .champ-row.all {
    border-bottom: 1px solid var(--border);
    font-weight: 600;
  }
  .champ-row.none .champ-name {
    color: var(--muted);
  }
  .champ-name {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .champ-value {
    font-size: 0.7rem;
    color: var(--muted);
  }
  .champ-count {
    min-width: 22px;
    text-align: right;
    font-weight: 600;
    color: var(--gold);
  }
  .champ-count.zero {
    color: #d06a6a;
  }
  main {
    flex: 1;
    min-width: 0;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 14px;
  }
  .empty {
    text-align: center;
    padding: 12vh 20px;
    color: var(--muted);
  }
  .empty .big {
    font-size: 1.2rem;
    color: var(--text);
  }

  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    display: grid;
    place-items: center;
    padding: 32px;
    z-index: 10;
  }
  .modal {
    max-width: min(1100px, 92vw);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    overflow: hidden;
  }
  .modal img {
    display: block;
    width: 100%;
    max-height: 74vh;
    object-fit: contain;
    background: #000;
  }
  .modal-loading {
    width: min(900px, 80vw);
    height: 300px;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .modal-info {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 16px;
    font-size: 0.95rem;
  }
</style>
