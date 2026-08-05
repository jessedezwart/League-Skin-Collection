<script lang="ts">
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import {
    formatMoney,
    rarityLabel,
    availabilityYear,
    type Currency,
    type Skin,
  } from "./types";

  let {
    skin,
    currency,
    onopen,
  }: { skin: Skin; currency: Currency; onopen: (skin: Skin) => void } =
    $props();

  let src = $state<string | null>(null);
  let failed = $state(false);

  // Only download the image once the card scrolls near the viewport.
  function lazy(node: HTMLElement) {
    const observer = new IntersectionObserver(
      async (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        observer.disconnect();
        try {
          const path = await invoke<string>("get_skin_image", {
            skinId: skin.skinId,
            kind: "loading",
          });
          src = convertFileSrc(path);
        } catch {
          failed = true;
        }
      },
      { rootMargin: "600px" }
    );
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  const rarity = $derived(rarityLabel(skin.rarity));
  const year = $derived(availabilityYear(skin.availabilityDate));
</script>

<button class="card" use:lazy onclick={() => onopen(skin)}>
  <div class="art">
    {#if src}
      <img {src} alt={skin.name} loading="lazy" />
    {:else if failed}
      <div class="placeholder">no image</div>
    {:else}
      <div class="placeholder shimmer"></div>
    {/if}
    {#if skin.isLegacy}
      <span class="tag legacy">Legacy</span>
    {/if}
    {#if rarity}
      <span class="tag rarity {skin.rarity}">{rarity}</span>
    {/if}
  </div>
  <div class="info">
    <span class="name" title={skin.name}>{skin.name}</span>
    <span class="champ">
      {skin.championName}
      {#if year}<span class="year">· {year}</span>{/if}
    </span>
    <span class="price" class:estimate={skin.valueSource !== "store"}>
      {#if skin.rp !== null}
        {skin.valueSource === "estimate" ? "~" : ""}{skin.rp.toLocaleString()} RP
        <span class="money">{formatMoney(skin.rp, currency)}</span>
      {:else}
        —
      {/if}
    </span>
  </div>
</button>

<style>
  .card {
    all: unset;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    transition: transform 0.15s ease, border-color 0.15s ease;
  }
  .card:hover,
  .card:focus-visible {
    transform: translateY(-3px);
    border-color: var(--gold);
  }
  .art {
    position: relative;
    aspect-ratio: 308 / 560;
    background: #0a0e13;
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .placeholder {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--muted);
    font-size: 0.75rem;
  }
  .shimmer {
    background: linear-gradient(110deg, #10161d 40%, #1a232d 50%, #10161d 60%);
    background-size: 200% 100%;
    animation: shimmer 1.4s infinite linear;
  }
  @keyframes shimmer {
    to {
      background-position-x: -200%;
    }
  }
  .tag {
    position: absolute;
    top: 6px;
    font-size: 0.65rem;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(4px);
  }
  .tag.legacy {
    left: 6px;
    color: #7fd8ff;
  }
  .tag.rarity {
    right: 6px;
    color: var(--gold);
  }
  .tag.kLegendary {
    color: #ff8266;
  }
  .tag.kUltimate {
    color: #ff66c4;
  }
  .tag.kMythic,
  .tag.kExalted,
  .tag.kTranscendent {
    color: #b48cff;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px 10px;
  }
  .name {
    font-weight: 600;
    font-size: 0.85rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .champ {
    color: var(--muted);
    font-size: 0.75rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .year {
    opacity: 0.7;
  }
  .price {
    margin-top: 4px;
    color: var(--gold);
    font-weight: 700;
    font-size: 0.8rem;
  }
  .price.estimate {
    color: var(--muted);
  }
  .money {
    float: right;
    color: var(--muted);
    font-weight: 400;
    font-size: 0.72rem;
  }
</style>
