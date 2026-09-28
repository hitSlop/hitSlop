<script lang="ts">
  import { Calendar, Popover } from "bits-ui";
  import { CalendarDate, today, getLocalTimeZone, type DateValue } from "@internationalized/date";
  import CalendarIcon from "@lucide/svelte/icons/calendar";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { formatDate, validDate } from "./schedule";

  let { value, onChange }: { value: string; onChange: (value: string) => void } = $props();
  let calendarOpen = $state(false);
  const longDate = $derived(formatDate(value, { month: "short", day: "numeric", year: "numeric" }));
  const calendarValue = $derived.by(() => {
    if (!validDate(value)) return today(getLocalTimeZone());
    const [y, m, d] = value.split("-").map(Number);
    return new CalendarDate(y, m, d);
  });

  function onDateSelect(date: DateValue | undefined) {
    if (date) {
      onChange(date.toString());
      calendarOpen = false;
    }
  }
</script>

<Popover.Root bind:open={calendarOpen}>
  <Popover.Trigger class="dateTrigger" aria-label="Planner date" data-slop-export="hide">
    <CalendarIcon size={14} />
    <span>{longDate}</span>
  </Popover.Trigger>
  <Popover.Portal>
    <Popover.Content class="calendarPopover" side="bottom" align="end" sideOffset={6} data-slop-export="hide">
      <Calendar.Root type="single" value={calendarValue} preventDeselect onValueChange={onDateSelect}>
        {#snippet children({ months, weekdays })}
          <Calendar.Header class="calHeader">
            <Calendar.PrevButton class="calNav" aria-label="Previous month">
              <ChevronLeft size={13} />
            </Calendar.PrevButton>
            <Calendar.Heading class="calTitle" />
            <Calendar.NextButton class="calNav" aria-label="Next month">
              <ChevronRight size={13} />
            </Calendar.NextButton>
          </Calendar.Header>
          {#each months as month}
            <Calendar.Grid class="calGrid">
              <Calendar.GridHead>
                <Calendar.GridRow class="calRow">
                  {#each weekdays as day}
                    <Calendar.HeadCell class="calHeadCell">{day.slice(0, 2)}</Calendar.HeadCell>
                  {/each}
                </Calendar.GridRow>
              </Calendar.GridHead>
              <Calendar.GridBody>
                {#each month.weeks as weekDates}
                  <Calendar.GridRow class="calRow">
                    {#each weekDates as date}
                      <Calendar.Cell {date} month={month.value} class="calCell">
                        <Calendar.Day class="calDay" />
                      </Calendar.Cell>
                    {/each}
                  </Calendar.GridRow>
                {/each}
              </Calendar.GridBody>
            </Calendar.Grid>
          {/each}
        {/snippet}
      </Calendar.Root>
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
