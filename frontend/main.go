package main

import (
	"log"
	"net/http"
	"os"
	"path/filepath"

	"cc-cost-frontend/handlers"
)

func main() {
	templateDir := findTemplateDir()
	backendURL := envOr("BACKEND_URL", "http://localhost:8080")

	h := handlers.New(templateDir, backendURL)

	staticDir := findStaticDir()

	mux := http.NewServeMux()
	mux.Handle("/static/", http.StripPrefix("/static/", http.FileServer(http.Dir(staticDir))))
	mux.HandleFunc("/", h.Overview)
	mux.HandleFunc("/sessions", h.Sessions)
	mux.HandleFunc("/projects", h.Projects)
	mux.HandleFunc("/settings", h.Settings)
	mux.HandleFunc("/rate-card", h.RateCard)

	addr := envOr("FRONTEND_ADDR", ":45123")
	log.Printf("Frontend listening on http://localhost%s", addr)
	if err := http.ListenAndServe(addr, mux); err != nil {
		log.Fatal(err)
	}
}

func findStaticDir() string {
	candidates := []string{
		"static",
		filepath.Join("frontend", "static"),
	}
	for _, path := range candidates {
		if _, err := os.Stat(path); err == nil {
			return path
		}
	}

	exe, err := os.Executable()
	if err == nil {
		exeDir := filepath.Dir(exe)
		for _, rel := range []string{"static", filepath.Join("frontend", "static")} {
			d := filepath.Join(exeDir, rel)
			if _, err := os.Stat(d); err == nil {
				return d
			}
		}
	}

	return filepath.Join("frontend", "static")
}

func findTemplateDir() string {
	candidates := []string{
		"templates",
		filepath.Join("frontend", "templates"),
	}
	for _, path := range candidates {
		if _, err := os.Stat(path); err == nil {
			return path
		}
	}

	exe, err := os.Executable()
	if err == nil {
		exeDir := filepath.Dir(exe)
		for _, rel := range []string{"templates", filepath.Join("frontend", "templates")} {
			d := filepath.Join(exeDir, rel)
			if _, err := os.Stat(d); err == nil {
				return d
			}
		}
	}

	return filepath.Join("frontend", "templates")
}

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
