const header = document.querySelector(".site-header");
const menuToggle = document.querySelector(".menu-toggle");
const tabs = document.querySelectorAll(".tab");
const panels = document.querySelectorAll(".download-card");

menuToggle?.addEventListener("click", () => {
  const isOpen = header.classList.toggle("nav-open");
  menuToggle.setAttribute("aria-expanded", String(isOpen));
});

document.querySelectorAll(".site-nav a").forEach((link) => {
  link.addEventListener("click", () => {
    header.classList.remove("nav-open");
    menuToggle?.setAttribute("aria-expanded", "false");
  });
});

tabs.forEach((tab) => {
  tab.addEventListener("click", () => {
    const target = tab.dataset.target;

    tabs.forEach((item) => item.classList.toggle("is-active", item === tab));
    panels.forEach((panel) => panel.classList.toggle("is-active", panel.id === target));
  });
});
