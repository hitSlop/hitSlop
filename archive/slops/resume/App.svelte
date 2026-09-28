<script lang="ts">
  import { Button } from "bits-ui";
  import Globe from "@lucide/svelte/icons/globe";
  import Mail from "@lucide/svelte/icons/mail";
  import MapPin from "@lucide/svelte/icons/map-pin";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { Slop, bindText, bindValue, useDocument } from "@hitslop/document/svelte";
  import schema, { badgeInitials } from "./schema";

  const doc = useDocument(schema);
  const initials = $derived(badgeInitials(doc.current.name, doc.current.initials));
</script>

<Slop>
  <main class="canvas" data-slop-selection="none">
    <article class="resume" aria-label="Resume for {doc.current.name}">
      <aside class="sidebar">
        <input class="monogram" aria-label="Initials" maxlength="3" value={initials} oninput={(event) => doc.fields.initials.set(event.currentTarget.value.toUpperCase())} />

        <section class="contactBlock" aria-labelledby="contact-heading">
          <h2 id="contact-heading" class="heading">Contact</h2>
          <label><Mail size={14} strokeWidth={1.7} /><span class="srOnly">Email</span><input class="field" aria-label="Email" use:bindValue={doc.fields.email} /></label>
          <label><MapPin size={14} strokeWidth={1.7} /><span class="srOnly">Location</span><input class="field" aria-label="Location" use:bindText={doc.fields.location} /></label>
          <label><Globe size={14} strokeWidth={1.7} /><span class="srOnly">Website</span><input class="field" aria-label="Website" use:bindValue={doc.fields.website} /></label>
        </section>

        <section class="skillsBlock" aria-labelledby="skills-heading">
          <div class="sectionHeading"><h2 id="skills-heading" class="heading">Skills</h2><Button.Root class="addCompact" data-slop-export="hide" aria-label="Add skill" onclick={() => doc.fields.skills.insert({ label: "New skill" })}><Plus size={11} strokeWidth={1.8} /></Button.Root></div>
          <div class="skills">
            {#each doc.current.skills as skill, index (skill.$id)}
              <span class="skill"><input aria-label="Skill {index + 1}" use:bindText={doc.at(skill).label} /><button class="skillRemove" data-slop-export="hide" aria-label="Remove {skill.label || "skill"}" onclick={() => doc.fields.skills.remove(skill.$id)}>×</button></span>
            {/each}
          </div>
        </section>

        <section class="educationBlock" aria-labelledby="education-heading">
          <div class="sectionHeading"><h2 id="education-heading" class="heading">Education</h2><Button.Root class="addCompact" data-slop-export="hide" aria-label="Add education" onclick={() => doc.fields.education.insert({ school: "School or program", program: "Degree or course", year: "Year" })}><Plus size={11} strokeWidth={1.8} /></Button.Root></div>
          <div class="educationList">
            {#each doc.current.education as item, index (item.$id)}
              <div class="educationItem">
                <input class="educationSchool" aria-label="School {index + 1}" use:bindText={doc.at(item).school} />
                <input class="field" aria-label="Degree or program {index + 1}" use:bindText={doc.at(item).program} />
                <div class="educationYear"><input class="field" aria-label="Graduation year {index + 1}" use:bindValue={doc.at(item).year} /><button class="removeCompact" data-slop-export="hide" aria-label="Remove {item.school || "education"}" onclick={() => doc.fields.education.remove(item.$id)}><Trash2 size={11} strokeWidth={1.65} /></button></div>
              </div>
            {/each}
          </div>
        </section>
      </aside>

      <section class="mainColumn">
        <header class="profile">
          <span class="eyebrow">Resume</span>
          <input class="name" aria-label="Full name" use:bindText={doc.fields.name} />
          <input class="role" aria-label="Professional role" use:bindText={doc.fields.role} />
          <textarea class="summary" aria-label="Professional summary" use:bindText={doc.fields.summary}></textarea>
        </header>

        <section class="experience" aria-labelledby="experience-heading">
          <div class="experienceHeading"><h2 id="experience-heading">Experience</h2><Button.Root class="addExperience" data-slop-export="hide" onclick={() => doc.fields.experience.insert({ role: "New role", company: "Company", period: "Year — now", summary: "A short description of the work and the impact you made." })}><Plus size={12} strokeWidth={1.8} /> Add role</Button.Root></div>
          <div class="experienceList">
            {#each doc.current.experience as item, index (item.$id)}
              <article class="experienceItem">
                <div class="experienceMeta"><input class="period" aria-label="Employment period {index + 1}" use:bindText={doc.at(item).period} /><button class="removeExperience" data-slop-export="hide" aria-label="Remove {item.role || "experience"}" onclick={() => doc.fields.experience.remove(item.$id)}><Trash2 size={12} strokeWidth={1.65} /></button></div>
                <div>
                  <div class="roleLine"><input class="jobRole" aria-label="Job title {index + 1}" use:bindText={doc.at(item).role} /><span>·</span><input class="company" aria-label="Company {index + 1}" use:bindText={doc.at(item).company} /></div>
                  <textarea class="experienceCopy" aria-label="Summary for {item.role || "experience"}" use:bindText={doc.at(item).summary}></textarea>
                </div>
              </article>
            {/each}
          </div>
        </section>
      </section>
    </article>
  </main>

  {#snippet exportView()}
    {@const data = doc.current}
    {@const skills = data.skills.filter((skill) => skill.label.trim())}
    {@const education = data.education.filter((item) => item.school.trim() || item.program.trim() || item.year.trim())}
    {@const experience = data.experience.filter((item) => item.role.trim() || item.company.trim() || item.period.trim() || item.summary.trim())}
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
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <article class="iconSheet">
        <div class="iconRail"></div>
        <div class="iconMonogram">{initials}</div>
      </article>
    </div>
  {/snippet}
</Slop>
