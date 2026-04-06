package pb

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"

	"github.com/jhump/protoreflect/desc/protoparse"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protodesc"
	"google.golang.org/protobuf/reflect/protoreflect"
	"google.golang.org/protobuf/types/dynamicpb"
)

type Schema struct {
	file     protoreflect.FileDescriptor
	services map[string]protoreflect.ServiceDescriptor
	messages map[string]protoreflect.MessageDescriptor
}

var (
	once   sync.Once
	schema *Schema
	err    error
)

func Load() (*Schema, error) {
	once.Do(func() {
		protoPath, findErr := discoverProtoPath()
		if findErr != nil {
			err = findErr
			return
		}

		parser := protoparse.Parser{
			ImportPaths: []string{filepath.Dir(protoPath)},
		}
		files, parseErr := parser.ParseFiles(filepath.Base(protoPath))
		if parseErr != nil {
			err = fmt.Errorf("parse proto: %w", parseErr)
			return
		}
		file, convertErr := protodesc.NewFile(files[0].AsFileDescriptorProto(), nil)
		if convertErr != nil {
			err = fmt.Errorf("convert descriptor: %w", convertErr)
			return
		}
		schema = &Schema{
			file:     file,
			services: map[string]protoreflect.ServiceDescriptor{},
			messages: map[string]protoreflect.MessageDescriptor{},
		}
		services := file.Services()
		for i := 0; i < services.Len(); i++ {
			service := services.Get(i)
			schema.services[string(service.FullName())] = service
		}
		messages := file.Messages()
		for i := 0; i < messages.Len(); i++ {
			message := messages.Get(i)
			schema.messages[string(message.FullName())] = message
		}
	})

	return schema, err
}

func MustLoad() *Schema {
	value, loadErr := Load()
	if loadErr != nil {
		panic(loadErr)
	}
	return value
}

func (s *Schema) Service(fullName string) protoreflect.ServiceDescriptor {
	return s.services[fullName]
}

func (s *Schema) Message(fullName string) protoreflect.MessageDescriptor {
	return s.messages[fullName]
}

func (s *Schema) NewMessage(fullName string) *dynamicpb.Message {
	return dynamicpb.NewMessage(s.Message(fullName))
}

func FieldByName(message protoreflect.Message, name string) protoreflect.FieldDescriptor {
	return message.Descriptor().Fields().ByName(protoreflect.Name(name))
}

func SetString(message protoreflect.Message, name string, value string) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfString(value))
}

func SetBool(message protoreflect.Message, name string, value bool) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfBool(value))
}

func SetInt64(message protoreflect.Message, name string, value int64) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfInt64(value))
}

func SetUint32(message protoreflect.Message, name string, value uint32) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfUint32(value))
}

func AddString(message protoreflect.Message, name string, value string) {
	field := FieldByName(message, name)
	list := message.Mutable(field).List()
	list.Append(protoreflect.ValueOfString(value))
}

func SetMessage(message protoreflect.Message, name string, child proto.Message) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfMessage(child.ProtoReflect()))
}

func SetStringMap(message protoreflect.Message, name string, values map[string]string) {
	field := FieldByName(message, name)
	target := message.Mutable(field).Map()
	for key, value := range values {
		target.Set(
			protoreflect.ValueOfString(key).MapKey(),
			protoreflect.ValueOfString(value),
		)
	}
}

func GetString(message protoreflect.Message, name string) string {
	field := FieldByName(message, name)
	return message.Get(field).String()
}

func GetBool(message protoreflect.Message, name string) bool {
	field := FieldByName(message, name)
	return message.Get(field).Bool()
}

func GetInt64(message protoreflect.Message, name string) int64 {
	field := FieldByName(message, name)
	return message.Get(field).Int()
}

func GetUint32(message protoreflect.Message, name string) uint32 {
	field := FieldByName(message, name)
	return uint32(message.Get(field).Uint())
}

func GetMessage(message protoreflect.Message, name string) protoreflect.Message {
	field := FieldByName(message, name)
	return message.Get(field).Message()
}

func discoverProtoPath() (string, error) {
	candidates := []string{
		filepath.Join("..", "proto", "openpinch.proto"),
		filepath.Join("proto", "openpinch.proto"),
		filepath.Join("..", "..", "proto", "openpinch.proto"),
	}

	for _, candidate := range candidates {
		if _, statErr := os.Stat(candidate); statErr == nil {
			return candidate, nil
		}
	}

	return "", fmt.Errorf("unable to locate proto/openpinch.proto from current working directory")
}
