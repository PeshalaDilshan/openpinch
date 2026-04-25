package main

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"log"
	"net"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/config"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/connectors"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/enginebridge"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/scheduler"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/server"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials"
)

func main() {
	cfg, err := config.Load()
	if err != nil {
		log.Fatalf("load config: %v", err)
	}

	bridge, err := enginebridge.New(cfg.Gateway.EngineEndpoint)
	if err != nil {
		log.Fatalf("engine bridge: %v", err)
	}
	defer bridge.Close()

	sched := scheduler.New(bridge)
	sched.Start()
	defer sched.Stop()

	registry := connectors.NewRegistry(cfg, bridge)

	service, err := server.New(cfg, bridge, sched, registry)
	if err != nil {
		log.Fatalf("gateway server: %v", err)
	}

	grpcOptions := []grpc.ServerOption{}
	if cfg.Gateway.TLS.Enabled {
		transport, err := loadTransportCredentials(cfg)
		if err != nil {
			log.Fatalf("load mTLS credentials: %v", err)
		}
		grpcOptions = append(grpcOptions, grpc.Creds(transport))
	}
	grpcServer := grpc.NewServer(grpcOptions...)
	if err := service.Register(grpcServer); err != nil {
		log.Fatalf("register gateway service: %v", err)
	}
	var webServer *http.Server
	if cfg.Gateway.Web.Enabled {
		webServer = &http.Server{
			Addr:    cfg.Gateway.Web.ListenAddress,
			Handler: service.HTTPHandler(),
		}
		go func() {
			log.Printf("OpenPinch web gateway listening on %s", cfg.Gateway.Web.ListenAddress)
			if err := webServer.ListenAndServe(); err != nil && err != http.ErrServerClosed {
				log.Printf("web gateway stopped: %v", err)
			}
		}()
	}

	listener, err := net.Listen("tcp", cfg.Gateway.ListenAddress)
	if err != nil {
		log.Fatalf("listen: %v", err)
	}

	go func() {
		if err := registry.Start(context.Background()); err != nil {
			log.Printf("connector registry stopped: %v", err)
		}
	}()

	if cfg.Gateway.TLS.Enabled {
		log.Printf("OpenPinch gateway listening with mTLS on %s", cfg.Gateway.ListenAddress)
	} else {
		log.Printf("OpenPinch gateway listening on %s", cfg.Gateway.ListenAddress)
	}

	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-signals
		log.Printf("gateway shutting down")
		registry.Stop()
		if webServer != nil {
			shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()
			_ = webServer.Shutdown(shutdownCtx)
		}
		grpcServer.GracefulStop()
	}()

	if err := grpcServer.Serve(listener); err != nil {
		log.Fatalf("grpc serve: %v", err)
	}
}

func loadTransportCredentials(cfg *config.Config) (credentials.TransportCredentials, error) {
	certificate, err := tls.LoadX509KeyPair(cfg.Gateway.TLS.CertFile, cfg.Gateway.TLS.KeyFile)
	if err != nil {
		return nil, err
	}

	tlsConfig := &tls.Config{
		Certificates: []tls.Certificate{certificate},
		MinVersion:   tls.VersionTLS13,
	}

	if cfg.Gateway.TLS.ClientCAFile != "" {
		caBytes, err := os.ReadFile(cfg.Gateway.TLS.ClientCAFile)
		if err != nil {
			return nil, err
		}
		pool := x509.NewCertPool()
		pool.AppendCertsFromPEM(caBytes)
		tlsConfig.ClientAuth = tls.RequireAndVerifyClientCert
		tlsConfig.ClientCAs = pool
	}

	return credentials.NewTLS(tlsConfig), nil
}
