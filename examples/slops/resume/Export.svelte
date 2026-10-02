<script lang="ts">
  import doc from "./schema";
  import Globe from "@lucide/svelte/icons/globe";
  import Mail from "@lucide/svelte/icons/mail";
  import MapPin from "@lucide/svelte/icons/map-pin";
  import { badgeInitials } from "./schema";

  const data = $derived(doc.current);
  const skills = $derived(data.skills.filter((skill) => skill.label.trim()));
  const education = $derived(data.education.filter((item) => item.school.trim() || item.program.trim() || item.year.trim()));
  const experience = $derived(data.experience.filter((item) => item.role.trim() || item.company.trim() || item.period.trim() || item.summary.trim()));

  const initials = $derived(badgeInitials(doc.current.name, doc.current.initials));
</script>

<article class="resume" aria-label="Exported resume for {data.name}">
  <aside class="sidebar">
    <span class="monogram">{initials}</span>

    <section class="contactBlock" aria-label="Contact">
      <h2 class="heading">Contact</h2>
      {#if data.email.trim()}<div><Mail size={14} strokeWidth={1.7} /><span class="field">{data.email}</span></div>{/if}
      {#if data.location.trim()}<div><MapPin size={14} strokeWidth={1.7} /><span class="field">{data.location}</span></div>{/if}
      {#if data.website.trim()}<div><Globe size={14} strokeWidth={1.7} /><span class="field">{data.website}</span></div>{/if}
    </section>

    {#if skills.length}
      <section class="skillsBlock" aria-label="Skills">
        <h2 class="heading">Skills</h2>
        <div class="skills">
          {#each skills as skill (skill.$id)}
            <span class="skill"><span>{skill.label}</span></span>
          {/each}
        </div>
      </section>
    {/if}

    {#if education.length}
      <section class="educationBlock" aria-label="Education">
        <h2 class="heading">Education</h2>
        <div class="educationList">
          {#each education as item (item.$id)}
            <div class="educationItem">
              <strong class="educationSchool">{item.school}</strong>
              {#if item.program.trim()}<span class="field">{item.program}</span>{/if}
              {#if item.year.trim()}<div class="educationYear"><span class="field">{item.year}</span></div>{/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}
  </aside>

  <section class="mainColumn">
    <header class="profile">
      <span class="eyebrow">Resume</span>
      <h1 class="name">{data.name}</h1>
      {#if data.role.trim()}<p class="role">{data.role}</p>{/if}
      {#if data.summary.trim()}<p class="summary">{data.summary}</p>{/if}
    </header>

    {#if experience.length}
      <section class="experience" aria-label="Experience">
        <div class="experienceHeading"><h2>Experience</h2></div>
        <div class="experienceList">
          {#each experience as item (item.$id)}
            <article class="experienceItem">
              <div class="experienceMeta"><span class="period">{item.period}</span></div>
              <div>
                <div class="roleLine"><span class="jobRole">{item.role}</span>{#if item.role.trim() && item.company.trim()}<span>·</span>{/if}<span class="company">{item.company}</span></div>
                {#if item.summary.trim()}<p class="experienceCopy">{item.summary}</p>{/if}
              </div>
            </article>
          {/each}
        </div>
      </section>
    {/if}
  </section>
</article>
