<script lang="ts">
  import { Button } from "bits-ui";
  import Globe from "@lucide/svelte/icons/globe";
  import Mail from "@lucide/svelte/icons/mail";
  import MapPin from "@lucide/svelte/icons/map-pin";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { bindText } from "@hitslop/document/svelte";
  import doc, { badgeInitials } from "./schema";
  const initials = $derived(badgeInitials(doc.current.name, doc.current.initials));
</script>

  <main class="canvas" data-slop-selection="none">
    <article class="resume" aria-label="Resume for {doc.current.name}">
      <aside class="sidebar">
        <input class="monogram" aria-label="Initials" maxlength="3" value={initials} oninput={(event) => doc.fields.initials.set(event.currentTarget.value.toUpperCase())} />

        <section class="contactBlock" aria-labelledby="contact-heading">
          <h2 id="contact-heading" class="heading">Contact</h2>
          <label><Mail size={14} strokeWidth={1.7} /><span class="srOnly">Email</span><input class="field" aria-label="Email" bind:value={doc.fields.email.value} /></label>
          <label><MapPin size={14} strokeWidth={1.7} /><span class="srOnly">Location</span><input class="field" aria-label="Location" use:bindText={doc.fields.location} /></label>
          <label><Globe size={14} strokeWidth={1.7} /><span class="srOnly">Website</span><input class="field" aria-label="Website" bind:value={doc.fields.website.value} /></label>
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
                <div class="educationYear"><input class="field" aria-label="Graduation year {index + 1}" bind:value={() => doc.at(item).year.value, (next) => { doc.at(item).year.value = next; }} /><button class="removeCompact" data-slop-export="hide" aria-label="Remove {item.school || "education"}" onclick={() => doc.fields.education.remove(item.$id)}><Trash2 size={11} strokeWidth={1.65} /></button></div>
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
