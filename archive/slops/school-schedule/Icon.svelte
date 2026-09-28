<script lang="ts">
  import Star from "@lucide/svelte/icons/star";
    const columns = [
    ["sage", "indigo", "terracotta"],
    ["slate", "rose", "sage"],
    ["terracotta", "teal", "indigo"],
    ["indigo", "sage", "amber"],
    ["teal", "terracotta", "rose"],
  ];
</script>

<div class={"sch-iconSurface"} aria-hidden="true">
  <div class={"sch-iconBoard"}>
    <span class={"sch-iconLabel"}>MY WEEK</span><span class={"sch-iconStar"}
      ><Star size={46} strokeWidth={2.5} /></span
    >
    <div class={"sch-iconGrid"}>
      {#each columns as col}<div class={"sch-iconColumn"}>
          {#each col as color}<span class={"sch-iconTile"} data-color={color}
            ></span>{/each}
        </div>{/each}
    </div>
  </div>
</div>
