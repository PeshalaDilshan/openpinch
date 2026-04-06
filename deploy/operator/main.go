package main

import (
	"encoding/json"
	"fmt"
	"os"
)

type OpenPinchAgentSpec struct {
	Name            string            `json:"name"`
	Namespace       string            `json:"namespace"`
	GatewayImage    string            `json:"gatewayImage"`
	PolicyConfigMap string            `json:"policyConfigMap"`
	ConnectorSecret string            `json:"connectorSecret"`
	Replicas        int               `json:"replicas"`
	Labels          map[string]string `json:"labels"`
}

func main() {
	spec := OpenPinchAgentSpec{
		Name:            "openpinch",
		Namespace:       "openpinch-system",
		GatewayImage:    "ghcr.io/peshaladilshan/openpinch-gateway:latest",
		PolicyConfigMap: "openpinch-policy-bundle",
		ConnectorSecret: "openpinch-connectors",
		Replicas:        1,
		Labels: map[string]string{
			"app.kubernetes.io/name":    "openpinch",
			"app.kubernetes.io/part-of": "openpinch",
		},
	}

	if err := json.NewEncoder(os.Stdout).Encode(spec); err != nil {
		fmt.Fprintf(os.Stderr, "encode operator spec: %v\n", err)
		os.Exit(1)
	}
}
