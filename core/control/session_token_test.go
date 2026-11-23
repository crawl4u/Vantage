package control

import (
	"context"
	"encoding/json"
	"strings"
	"testing"
)

type tokenServices struct {
	stubServices
	token string
}

func (s *tokenServices) SessionToken(context.Context) (string, error) {
	if s.signedOut {
		return "", ErrSignedOut
	}
	return s.token, nil
}

func TestSessionTokenIsServedOnlyByItsOwnMethodAndNeverInStatusReplies(t *testing.T) {
	store := NewStore()
	store.SetAuth(AuthV1{State: AuthSignedIn, Gamertag: "Steve"})
	stub := &tokenServices{token: "secret-token"}
	dir := startServices(t, store, stub)

	var result sessionTokenResultV1
	if reply := rpc(t, dir, methodSessionToken, ""); reply.Error != nil || json.Unmarshal(reply.Result, &result) != nil || result.AccessToken != "secret-token" {
		t.Fatalf("session_token = %+v %+v", reply.Error, result)
	}
	for _, method := range []string{methodAccountStatus, methodEvents} {
		if raw := call(t, dir, method, ""); strings.Contains(string(raw), "secret-token") {
			t.Fatalf("%s reply carries the token: %s", method, raw)
		}
	}
	if reply := rpc(t, dir, methodSessionToken, `{"x":1}`); reply.Error == nil || reply.Error.Code != -32602 {
		t.Fatalf("params error = %+v", reply.Error)
	}
	stub.signedOut = true
	if reply := rpc(t, dir, methodSessionToken, ""); reply.Error == nil || reply.Error.Code != codeSignedOut {
		t.Fatalf("signed out error = %+v", reply.Error)
	}
}

func TestSessionTokenNeedsABackendThatCanServeIt(t *testing.T) {
	dir := startServices(t, NewStore(), &stubServices{})
	if reply := rpc(t, dir, methodSessionToken, ""); reply.Error == nil || reply.Error.Code != codeServicesDisabled {
		t.Fatalf("error = %+v", reply.Error)
	}
	dir = startServices(t, NewStore(), nil)
	if reply := rpc(t, dir, methodSessionToken, ""); reply.Error == nil || reply.Error.Code != codeServicesDisabled {
		t.Fatalf("no services error = %+v", reply.Error)
	}
}
