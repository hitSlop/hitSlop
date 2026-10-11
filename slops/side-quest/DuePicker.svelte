<script lang="ts">
  import { DatePicker } from "bits-ui";
  import { parseDate, type DateValue } from "@internationalized/date";
  import CalendarDays from "@lucide/svelte/icons/calendar-days";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";

  let { value, onChange }: { value: string; onChange: (iso: string) => void } = $props();

  const date = $derived.by(() => { try { return parseDate(value); } catch { return undefined; } });

  function change(next: DateValue | undefined) {
    if (next && next.toString() !== value) onChange(next.toString());
  }
</script>

<DatePicker.Root value={date} onValueChange={change} weekdayFormat="narrow" fixedWeeks>
  <DatePicker.Input class="sq-date-input" aria-label="Due date">
    {#snippet children({ segments })}
      {#each segments as { part, value: text }, index (index)}
        <DatePicker.Segment {part} class="sq-date-segment">{text}</DatePicker.Segment>
      {/each}
      <DatePicker.Trigger class="sq-date-trigger" aria-label="Open calendar"><CalendarDays size={14} /></DatePicker.Trigger>
    {/snippet}
  </DatePicker.Input>
  <DatePicker.Portal>
    <DatePicker.Content class="sq-date-pop" data-slop-export="hide" sideOffset={6} align="end">
      <DatePicker.Calendar class="sq-date-cal">
        {#snippet children({ months, weekdays })}
          <DatePicker.Header class="sq-date-head">
            <DatePicker.PrevButton class="sq-date-nav" aria-label="Previous month"><ChevronLeft size={14} /></DatePicker.PrevButton>
            <DatePicker.Heading class="sq-date-heading" />
            <DatePicker.NextButton class="sq-date-nav" aria-label="Next month"><ChevronRight size={14} /></DatePicker.NextButton>
          </DatePicker.Header>
          {#each months as month (month.value.toString())}
            <DatePicker.Grid class="sq-date-grid">
              <DatePicker.GridHead>
                <DatePicker.GridRow class="sq-date-row">
                  {#each weekdays as day, index (index)}<DatePicker.HeadCell class="sq-date-weekday">{day}</DatePicker.HeadCell>{/each}
                </DatePicker.GridRow>
              </DatePicker.GridHead>
              <DatePicker.GridBody>
                {#each month.weeks as week, index (index)}
                  <DatePicker.GridRow class="sq-date-row">
                    {#each week as day (day.toString())}
                      <DatePicker.Cell date={day} month={month.value} class="sq-date-cell">
                        <DatePicker.Day class="sq-date-day" />
                      </DatePicker.Cell>
                    {/each}
                  </DatePicker.GridRow>
                {/each}
              </DatePicker.GridBody>
            </DatePicker.Grid>
          {/each}
        {/snippet}
      </DatePicker.Calendar>
    </DatePicker.Content>
  </DatePicker.Portal>
</DatePicker.Root>
