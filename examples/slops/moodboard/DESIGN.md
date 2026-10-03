# Moodboard design

One job: hold a handful of images you are thinking about and let you move through them.

The object is a dark violet room with a soft glow. The stage is a WebGL tunnel (Motion Core's
Infinite Gallery on OGL) where images drift toward you with depth fade and blur; scroll,
drag or arrow keys move through it, and it slowly drifts on its own unless motion is
reduced. Under it, a strip lists every image with an editable caption and a remove button,
so the board is readable and editable without the 3D view. The Add button leans toward the
cursor (Motion Core's Magnetic, via GSAP). Dropping files anywhere also adds them.

Saved: the title, and each tile's attachment reference and caption. Image bytes are
host-owned attachments (JPEG, PNG or WebP, up to 10 MB each, 24 per board); templates ship
empty. Object URLs, scroll position and the drift are never saved. Export is a contact
sheet of the images with captions, not the live canvas; the icon is a fan of three cards.
